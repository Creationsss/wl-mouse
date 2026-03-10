use anyhow::{bail, Result};

use crate::consts::*;

pub struct HidTransport<'a> {
	device: &'a hidapi::HidDevice,
	pub hid_index: u8,
}

impl<'a> HidTransport<'a> {
	pub fn new(device: &'a hidapi::HidDevice) -> Self {
		Self {
			device,
			hid_index: 0,
		}
	}

	pub fn send_and_recv(&self, data: &[u8; REPORT_SIZE]) -> Result<[u8; REPORT_SIZE]> {
		let send_buf: Vec<u8> = std::iter::once(REPORT_ID)
			.chain(data.iter().copied())
			.collect();
		let cmd_byte = data[5];

		for attempt in 0..MAX_RETRIES {
			self.device.send_feature_report(&send_buf)?;

			std::thread::sleep(std::time::Duration::from_millis(30));

			let mut buf = [0u8; REPORT_SIZE + 1];
			buf[0] = REPORT_ID;
			self.device.get_feature_report(&mut buf)?;

			let resp = &buf[1..];
			let check_idx = 1 - self.hid_index as usize;

			if resp[check_idx] == RESPONSE_OK && resp[5] == cmd_byte {
				let mut result = [0u8; REPORT_SIZE];
				result.copy_from_slice(resp);
				return Ok(result);
			}

			if resp[check_idx] == RESPONSE_SLEEPING
				|| (resp[check_idx] == RESPONSE_OK && resp[5] != cmd_byte)
			{
				if attempt == 0 {
					eprintln!("Mouse is asleep, waiting for it to wake up...");
				}
				std::thread::sleep(std::time::Duration::from_millis(500));
				continue;
			}

			if attempt < MAX_RETRIES - 1 {
				std::thread::sleep(std::time::Duration::from_millis(50));
			}
		}
		bail!("no response from mouse (is it asleep or out of range?)")
	}

	pub fn detect_hid_index(&mut self) -> Result<()> {
		let cmd = build_get_firmware(0x02);
		let send_buf: Vec<u8> = std::iter::once(REPORT_ID)
			.chain(cmd.iter().copied())
			.collect();

		for attempt in 0..MAX_RETRIES {
			self.device.send_feature_report(&send_buf)?;
			std::thread::sleep(std::time::Duration::from_millis(30));

			let mut buf = [0u8; REPORT_SIZE + 1];
			buf[0] = REPORT_ID;
			self.device.get_feature_report(&mut buf)?;

			let resp = &buf[1..];
			if resp[0] == RESPONSE_OK {
				self.hid_index = 1;
				return Ok(());
			} else if resp[1] == RESPONSE_OK {
				self.hid_index = 0;
				return Ok(());
			}

			if resp[0] == RESPONSE_SLEEPING || resp[1] == RESPONSE_SLEEPING {
				if attempt == 0 {
					eprintln!("Mouse is asleep, waiting for it to wake up...");
				}
				std::thread::sleep(std::time::Duration::from_millis(500));
				continue;
			}
		}
		bail!("no response from mouse (is it asleep or out of range?)")
	}

	pub fn send_only(&self, data: &[u8; REPORT_SIZE]) -> Result<()> {
		self.device.send_feature_report(
			&std::iter::once(REPORT_ID)
				.chain(data.iter().copied())
				.collect::<Vec<u8>>(),
		)?;
		Ok(())
	}
}

fn build_profile_get(len: u8, page: u8, cmd: u8, profile: u8) -> [u8; REPORT_SIZE] {
	let mut buf = [0u8; REPORT_SIZE];
	buf[2] = 0x02;
	buf[3] = len;
	buf[4] = page;
	buf[5] = cmd;
	buf[6] = profile;
	buf
}

fn build_profile_set(len: u8, page: u8, cmd: u8, profile: u8, value: u8) -> [u8; REPORT_SIZE] {
	let mut buf = build_profile_get(len, page, cmd, profile);
	buf[7] = value;
	buf
}

pub fn resp_u8(resp: &[u8; REPORT_SIZE], hid_index: u8) -> u8 {
	resp[(8 - hid_index) as usize]
}

pub fn resp_bool(resp: &[u8; REPORT_SIZE], hid_index: u8) -> bool {
	resp_u8(resp, hid_index) != 0
}

pub fn build_get_firmware(device_id: u8) -> [u8; REPORT_SIZE] {
	let mut buf = [0u8; REPORT_SIZE];
	buf[2] = device_id;
	buf[3] = 0x10;
	buf[4] = 0x00;
	buf[5] = 0x81;
	buf
}

pub fn parse_firmware(resp: &[u8; REPORT_SIZE], hid_index: u8) -> String {
	let o = hid_index as usize;
	format!(
		"{}.{}.{}.{}",
		resp[7 - o],
		resp[8 - o],
		resp[9 - o],
		resp[10 - o]
	)
}

pub fn build_get_battery() -> [u8; REPORT_SIZE] {
	let mut buf = [0u8; REPORT_SIZE];
	buf[2] = 0x02;
	buf[3] = 0x02;
	buf[4] = 0x00;
	buf[5] = 0x83;
	buf
}

pub fn parse_battery(resp: &[u8; REPORT_SIZE], hid_index: u8) -> (u8, bool) {
	let idx = (7 - hid_index) as usize;
	let (a, b) = (resp[idx], resp[idx + 1]);
	if a > 1 {
		(a, b != 0)
	} else {
		(b, a != 0)
	}
}

pub fn build_get_polling_rate(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x02, 0x01, 0x80, profile)
}

pub fn parse_polling_rate(resp: &[u8; REPORT_SIZE], hid_index: u8) -> u16 {
	match resp_u8(resp, hid_index) {
		8 => 125,
		4 => 250,
		2 => 500,
		1 | 16 => 1000,
		32 => 2000,
		64 => 4000,
		128 => 8000,
		raw => raw as u16,
	}
}

pub fn build_set_polling_rate(profile: u8, rate: u16) -> [u8; REPORT_SIZE] {
	let raw = match rate {
		125 => 8,
		250 => 4,
		500 => 2,
		1000 => 1,
		2000 => 32,
		4000 => 64,
		8000 => 128,
		_ => 1,
	};
	build_profile_set(0x02, 0x01, 0x00, profile, raw)
}

pub fn build_get_active_dpi(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x02, 0x01, 0x82, profile)
}

pub fn build_get_dpi_stages(profile: u8, count: u8) -> [u8; REPORT_SIZE] {
	build_profile_set(0x0A, 0x01, 0x81, profile, count)
}

pub fn parse_dpi_stages(resp: &[u8; REPORT_SIZE], count: u8, hid_index: u8) -> Vec<(u16, u16)> {
	let mut stages = Vec::new();
	let o = hid_index as usize;
	for i in 0..count as usize {
		let xi = 9 + i * 4 - o;
		if xi + 3 >= REPORT_SIZE {
			break;
		}
		let x = ((resp[xi] as u16) << 8) | resp[xi + 1] as u16;
		let y = ((resp[xi + 2] as u16) << 8) | resp[xi + 3] as u16;
		stages.push((x, y));
	}
	stages
}

pub fn build_set_dpi_stages(profile: u8, stages: &[(u16, u16)]) -> [u8; REPORT_SIZE] {
	let mut buf = [0u8; REPORT_SIZE];
	buf[2] = 0x02;
	buf[3] = 0x1A;
	buf[4] = 0x01;
	buf[5] = 0x01;
	buf[6] = profile;
	buf[7] = stages.len() as u8;
	for (i, (x, y)) in stages.iter().enumerate() {
		let off = 8 + i * 4;
		buf[off] = (*x >> 8) as u8;
		buf[off + 1] = *x as u8;
		buf[off + 2] = (*y >> 8) as u8;
		buf[off + 3] = *y as u8;
	}
	buf
}

pub fn build_set_active_dpi(profile: u8, stage: u8) -> [u8; REPORT_SIZE] {
	build_profile_set(0x02, 0x01, 0x02, profile, stage)
}

pub fn build_get_lod(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x02, 0x01, 0x88, profile)
}

pub fn parse_lod(resp: &[u8; REPORT_SIZE], hid_index: u8) -> f32 {
	let raw = resp_u8(resp, hid_index);
	if raw & 0x80 != 0 {
		(raw & 0x7F) as f32 / 10.0
	} else {
		raw as f32
	}
}

pub fn build_set_lod(profile: u8, lod: f32) -> [u8; REPORT_SIZE] {
	let raw = if lod < 1.0 {
		((lod * 10.0) as u8) | 0x80
	} else {
		lod as u8
	};
	build_profile_set(0x02, 0x01, 0x08, profile, raw)
}

pub fn build_get_debounce(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x02, 0x00, 0x88, profile)
}

pub fn build_set_debounce(profile: u8, ms: u8) -> [u8; REPORT_SIZE] {
	build_profile_set(0x02, 0x00, 0x08, profile, ms)
}

pub fn build_get_angle_snap(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x02, 0x01, 0x84, profile)
}

pub fn build_set_angle_snap(profile: u8, enabled: bool) -> [u8; REPORT_SIZE] {
	build_profile_set(0x02, 0x01, 0x04, profile, enabled as u8)
}

pub fn build_get_motion_sync(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x02, 0x01, 0x89, profile)
}

pub fn build_set_motion_sync(profile: u8, enabled: bool) -> [u8; REPORT_SIZE] {
	build_profile_set(0x02, 0x01, 0x09, profile, enabled as u8)
}

pub fn build_get_sleep_time(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x03, 0x00, 0x87, profile)
}

pub fn parse_sleep_time(resp: &[u8; REPORT_SIZE], hid_index: u8) -> u16 {
	let idx = (8 - hid_index) as usize;
	((resp[idx] as u16) << 8) | resp[idx + 1] as u16
}

pub fn build_set_sleep_time(profile: u8, seconds: u16) -> [u8; REPORT_SIZE] {
	let mut buf = build_profile_get(0x03, 0x00, 0x07, profile);
	buf[7] = (seconds >> 8) as u8;
	buf[8] = seconds as u8;
	buf
}

pub fn build_get_profile_id() -> [u8; REPORT_SIZE] {
	let mut buf = [0u8; REPORT_SIZE];
	buf[2] = 0x02;
	buf[3] = 0x01;
	buf[4] = 0x00;
	buf[5] = 0x85;
	buf
}

pub fn build_get_angle_tune(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x02, 0x01, 0x94, profile)
}

pub fn build_set_angle_tune(profile: u8, value: i8) -> [u8; REPORT_SIZE] {
	let raw = if value < 0 {
		255 - value.unsigned_abs() + 1
	} else {
		value as u8
	};
	build_profile_set(0x02, 0x01, 0x14, profile, raw)
}

pub fn build_get_ripple_control(profile: u8) -> [u8; REPORT_SIZE] {
	build_profile_get(0x02, 0x01, 0x8A, profile)
}

pub fn build_set_ripple_control(profile: u8, enabled: bool) -> [u8; REPORT_SIZE] {
	build_profile_set(0x02, 0x01, 0x0A, profile, enabled as u8)
}

pub fn build_get_sn() -> [u8; REPORT_SIZE] {
	let mut buf = [0u8; REPORT_SIZE];
	buf[2] = 0x02;
	buf[3] = 0x14;
	buf[4] = 0x00;
	buf[5] = 0x82;
	buf
}

pub fn parse_sn(resp: &[u8; REPORT_SIZE], hid_index: u8) -> String {
	let start = (9 - hid_index) as usize;
	let bytes: Vec<u8> = resp[start..]
		.iter()
		.copied()
		.take_while(|&b| b != 0 && b != 0xFF && b.is_ascii_graphic())
		.collect();
	String::from_utf8_lossy(&bytes).to_string()
}

pub fn build_factory_reset() -> [u8; REPORT_SIZE] {
	let mut buf = [0u8; REPORT_SIZE];
	buf[2] = 0x02;
	buf[3] = 0x02;
	buf[4] = 0x00;
	buf[5] = 0x00;
	buf[6] = 0xC0;
	buf[7] = 0x01;
	buf
}
