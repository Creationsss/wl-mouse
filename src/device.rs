use anyhow::{bail, Result};

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
}
