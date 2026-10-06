use anyhow::{bail, Context, Result};

use crate::consts::*;
use crate::protocol::*;

pub struct Device {
	hid: hidapi::HidDevice,
	pub name: String,
	_pid: u16,
	hid_index: u8,
}

fn pid_name(pid: u16) -> String {
	KNOWN_PIDS
		.iter()
		.find(|(p, _)| *p == pid)
		.map(|(_, n)| n.to_string())
		.unwrap_or_else(|| format!("Unknown ({pid:#06x})"))
}

pub fn list_devices() -> Result<Vec<(String, u16, String)>> {
	let api = hidapi::HidApi::new()?;
	let mut found = Vec::new();

	for info in api.device_list() {
		if info.vendor_id() != WL_VID {
			continue;
		}
		let pid = info.product_id();
		let path = info.path().to_string_lossy().to_string();

		if info.usage_page() == 0xFFFF && info.usage() == 0 {
			found.push((pid_name(pid), pid, path));
		}
	}

	found.sort_by_key(|f| f.1);
	found.dedup_by(|a, b| a.1 == b.1);

	let wired_pids: Vec<u16> = found.iter().map(|f| f.1).collect();
	found.retain(|f| {
		let is_dongle = f.1 % 2 == 0;
		!is_dongle || !wired_pids.contains(&(f.1 + 1))
	});

	Ok(found)
}

fn detect_device() -> Result<(String, u16, String)> {
	let devices = list_devices()?;
	match devices.len() {
		0 => bail!("no WLmouse device found (VID:{WL_VID:#06x}). Is it plugged in?"),
		1 => Ok(devices.into_iter().next().unwrap()),
		_ => {
			eprintln!("Multiple WLmouse devices found:");
			for (i, (name, pid, path)) in devices.iter().enumerate() {
				eprintln!("  {}: {} (PID:{pid:#06x}) at {path}", i + 1, name);
			}
			bail!("use --device <path> to select one")
		}
	}
}

impl Device {
	pub fn open(path: Option<&str>) -> Result<Self> {
		let (name, pid, dev_path) = match path {
			Some(p) => {
				let api = hidapi::HidApi::new()?;
				let pid = api
					.device_list()
					.find(|i| i.path().to_string_lossy() == p)
					.map(|i| i.product_id())
					.unwrap_or(0);
				(pid_name(pid), pid, p.to_string())
			}
			None => detect_device()?,
		};

		let api = hidapi::HidApi::new()?;
		let hid = api.open_path(&std::ffi::CString::new(dev_path.as_str())?)?;

		let mut dev = Device {
			hid,
			name,
			_pid: pid,
			hid_index: 0,
		};

		let mut transport = HidTransport::new(&dev.hid);
		transport.detect_hid_index()?;
		dev.hid_index = transport.hid_index;

		Ok(dev)
	}

	fn transport(&self) -> HidTransport<'_> {
		let mut t = HidTransport::new(&self.hid);
		t.hid_index = self.hid_index;
		t
	}

	pub fn firmware_version(&self) -> Result<String> {
		let resp = self.transport().send_and_recv(&build_get_firmware(0x02))?;
		Ok(parse_firmware(&resp, self.hid_index))
	}

	pub fn dongle_firmware_version(&self) -> Result<String> {
		let resp = self.transport().send_and_recv(&build_get_firmware(0x00))?;
		Ok(parse_firmware(&resp, self.hid_index))
	}

	pub fn battery(&self) -> Result<(u8, bool)> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_battery())?;
		Ok(parse_battery(&resp, transport.hid_index))
	}

	pub fn serial_number(&self) -> Result<String> {
		let resp = self.transport().send_and_recv(&build_get_sn())?;
		Ok(parse_sn(&resp, self.hid_index))
	}

	pub fn active_profile(&self) -> Result<u8> {
		let resp = self.transport().send_and_recv(&build_get_profile_id())?;
		Ok(resp[(7 - self.hid_index) as usize])
	}

	pub fn polling_rate(&self, profile: u8) -> Result<u16> {
		let resp = self
			.transport()
			.send_and_recv(&build_get_polling_rate(profile))?;
		Ok(parse_polling_rate(&resp, self.hid_index))
	}

	pub fn set_polling_rate(&self, profile: u8, rate: u16) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_polling_rate(profile, rate))?;
		Ok(())
	}

	pub fn dpi_stages(&self, profile: u8, count: u8) -> Result<(u8, Vec<(u16, u16)>)> {
		let active_resp = self
			.transport()
			.send_and_recv(&build_get_active_dpi(profile))?;
		let active_stage = resp_u8(&active_resp, self.hid_index).saturating_sub(1);

		let stages_resp = self
			.transport()
			.send_and_recv(&build_get_dpi_stages(profile, count))?;
		let stages = parse_dpi_stages(&stages_resp, count, self.hid_index);

		Ok((active_stage, stages))
	}

	pub fn set_dpi_stages(&self, profile: u8, stages: &[(u16, u16)]) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_dpi_stages(profile, stages))?;
		Ok(())
	}

	pub fn set_active_dpi(&self, profile: u8, stage: u8) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_active_dpi(profile, stage))?;
		Ok(())
	}

	pub fn lod(&self, profile: u8) -> Result<f32> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_lod(profile))?;
		Ok(parse_lod(&resp, transport.hid_index))
	}

	pub fn set_lod(&self, profile: u8, lod: f32) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_lod(profile, lod))?;
		Ok(())
	}

	pub fn debounce(&self, profile: u8) -> Result<u8> {
		let resp = self
			.transport()
			.send_and_recv(&build_get_debounce(profile))?;
		Ok(resp_u8(&resp, self.hid_index))
	}

	pub fn set_debounce(&self, profile: u8, ms: u8) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_debounce(profile, ms))?;
		Ok(())
	}

	pub fn angle_snap(&self, profile: u8) -> Result<bool> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_angle_snap(profile))?;
		Ok(resp_bool(&resp, transport.hid_index))
	}

	pub fn set_angle_snap(&self, profile: u8, enabled: bool) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_angle_snap(profile, enabled))?;
		Ok(())
	}

	pub fn motion_sync(&self, profile: u8) -> Result<bool> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_motion_sync(profile))?;
		Ok(resp_bool(&resp, transport.hid_index))
	}

	pub fn set_motion_sync(&self, profile: u8, enabled: bool) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_motion_sync(profile, enabled))?;
		Ok(())
	}

	pub fn angle_tune(&self, profile: u8) -> Result<i8> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_angle_tune(profile))?;
		Ok(resp_u8(&resp, transport.hid_index) as i8)
	}

	pub fn set_angle_tune(&self, profile: u8, value: i8) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_angle_tune(profile, value))?;
		Ok(())
	}

	pub fn hyper_mode(&self, profile: u8) -> Result<bool> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_hyper_mode(profile))?;
		Ok(resp_bool(&resp, transport.hid_index))
	}

	pub fn set_hyper_mode(&self, profile: u8, enabled: bool) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_hyper_mode(profile, enabled))?;
		Ok(())
	}

	pub fn turbo_mode(&self, profile: u8) -> Result<bool> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_turbo_mode(profile))?;
		Ok(resp_bool(&resp, transport.hid_index))
	}

	pub fn set_turbo_mode(&self, profile: u8, enabled: bool) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_turbo_mode(profile, enabled))?;
		Ok(())
	}

	pub fn ripple_control(&self, profile: u8) -> Result<bool> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_ripple_control(profile))?;
		Ok(resp_bool(&resp, transport.hid_index))
	}

	pub fn set_ripple_control(&self, profile: u8, enabled: bool) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_ripple_control(profile, enabled))?;
		Ok(())
	}

	pub fn sleep_time(&self, profile: u8) -> Result<u16> {
		let transport = self.transport();
		let resp = transport.send_and_recv(&build_get_sleep_time(profile))?;
		Ok(parse_sleep_time(&resp, transport.hid_index))
	}

	pub fn set_sleep_time(&self, profile: u8, seconds: u16) -> Result<()> {
		self.transport()
			.send_and_recv(&build_set_sleep_time(profile, seconds))?;
		Ok(())
	}

	pub fn factory_reset(&self) -> Result<()> {
		self.transport().send_only(&build_factory_reset())?;
		Ok(())
	}

	pub fn pid(&self) -> u16 {
		self._pid
	}

	pub fn enter_bootloader(&self, device_id: u8) {
		let _ = self.transport().send_only(&build_enter_bl(device_id));
	}

	pub fn profile_count(&self) -> Result<u8> {
		let resp = self.transport().send_and_recv(&build_get_profile_count())?;
		Ok(resp[(7 - self.hid_index) as usize])
	}

	pub fn set_active_profile(&self, id: u8) -> Result<()> {
		self.transport().send_and_recv(&build_set_profile_id(id))?;
		Ok(())
	}
}

pub struct ProfileSnapshot {
	profile: u8,
	polling_rate: Option<u16>,
	dpi: Option<(u8, Vec<(u16, u16)>)>,
	lod: Option<f32>,
	debounce: Option<u8>,
	angle_snap: Option<bool>,
	motion_sync: Option<bool>,
	angle_tune: Option<i8>,
	ripple_control: Option<bool>,
	sleep_time: Option<u16>,
	hyper_mode: Option<bool>,
	turbo_mode: Option<bool>,
}

impl ProfileSnapshot {
	fn take(dev: &Device, profile: u8) -> Self {
		ProfileSnapshot {
			profile,
			polling_rate: dev.polling_rate(profile).ok(),
			dpi: dev.dpi_stages(profile, 6).ok(),
			lod: dev.lod(profile).ok(),
			debounce: dev.debounce(profile).ok(),
			angle_snap: dev.angle_snap(profile).ok(),
			motion_sync: dev.motion_sync(profile).ok(),
			angle_tune: dev.angle_tune(profile).ok(),
			ripple_control: dev.ripple_control(profile).ok(),
			sleep_time: dev.sleep_time(profile).ok(),
			hyper_mode: dev.hyper_mode(profile).ok(),
			turbo_mode: dev.turbo_mode(profile).ok(),
		}
	}

	fn restore(&self, dev: &Device) -> Result<()> {
		let p = self.profile;
		if let Some(r) = self.polling_rate {
			dev.set_polling_rate(p, r)?;
		}
		if let Some((active, stages)) = &self.dpi {
			dev.set_dpi_stages(p, stages)?;
			dev.set_active_dpi(p, active + 1)?;
		}
		if let Some(v) = self.lod {
			dev.set_lod(p, v)?;
		}
		if let Some(v) = self.debounce {
			dev.set_debounce(p, v)?;
		}
		if let Some(v) = self.angle_snap {
			dev.set_angle_snap(p, v)?;
		}
		if let Some(v) = self.motion_sync {
			dev.set_motion_sync(p, v)?;
		}
		if let Some(v) = self.angle_tune {
			dev.set_angle_tune(p, v)?;
		}
		if let Some(v) = self.ripple_control {
			dev.set_ripple_control(p, v)?;
		}
		if let Some(v) = self.sleep_time {
			dev.set_sleep_time(p, v)?;
		}
		if let Some(v) = self.hyper_mode {
			dev.set_hyper_mode(p, v)?;
		}
		if let Some(v) = self.turbo_mode {
			dev.set_turbo_mode(p, v)?;
		}
		Ok(())
	}
}

pub struct Snapshot {
	pid: u16,
	active_profile: u8,
	profiles: Vec<ProfileSnapshot>,
}

impl Snapshot {
	pub fn take(dev: &Device) -> Option<Self> {
		let active_profile = dev.active_profile().ok()?;
		let count = dev
			.profile_count()
			.ok()
			.filter(|n| (1..=8).contains(n))
			.unwrap_or(3);
		let profiles = (1..=count).map(|p| ProfileSnapshot::take(dev, p)).collect();
		Some(Snapshot {
			pid: dev.pid(),
			active_profile,
			profiles,
		})
	}

	pub fn profile_count(&self) -> usize {
		self.profiles.len()
	}

	pub fn restore(&self) -> Result<()> {
		let (_, _, path) = list_devices()?
			.into_iter()
			.find(|d| d.1 == self.pid)
			.with_context(|| format!("device {:#06x} not found after update", self.pid))?;
		let dev = Device::open(Some(&path))?;
		for p in &self.profiles {
			p.restore(&dev)?;
		}
		dev.set_active_profile(self.active_profile)?;
		Ok(())
	}
}
