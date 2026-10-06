use anyhow::{bail, Result};

use crate::consts::*;
use crate::types::Delays;

pub struct Bl {
	dev: hidapi::HidDevice,
	hid_index: usize,
}

impl Bl {
	pub fn new(dev: hidapi::HidDevice) -> Self {
		Self { dev, hid_index: 0 }
	}

	fn send(&self, data: &[u8; REPORT_SIZE]) -> Result<()> {
		let buf: Vec<u8> = std::iter::once(REPORT_ID)
			.chain(data.iter().copied())
			.collect();
		self.dev.send_feature_report(&buf)?;
		Ok(())
	}

	fn read(&self) -> Result<[u8; REPORT_SIZE]> {
		let mut buf = [0u8; REPORT_SIZE + 1];
		buf[0] = REPORT_ID;
		self.dev.get_feature_report(&mut buf)?;
		let mut out = [0u8; REPORT_SIZE];
		out.copy_from_slice(&buf[1..]);
		Ok(out)
	}

	/// send command, wait for OK, resend periodically (just like the web app).
	fn cmd_ack(
		&self,
		cmd: &[u8; REPORT_SIZE],
		delay_ms: u64,
		tries: u32,
		want_echo: bool,
	) -> Result<[u8; REPORT_SIZE]> {
		self.send(cmd)?;
		for i in 0..tries {
			std::thread::sleep(std::time::Duration::from_millis(delay_ms));
			let resp = self.read()?;
			let status = resp[1 - self.hid_index];
			if (status == BL_STATUS_OK || status == BL_STATUS_OK2)
				&& (!want_echo || resp[5 - self.hid_index] == BL_PAGE)
			{
				return Ok(resp);
			}
			if i % 20 == 19 {
				let _ = self.send(cmd);
			}
		}
		bail!("bootloader did not acknowledge command {:#04x}", cmd[5])
	}

	/// hid_index and BL version.
	pub fn fw_ver(&mut self, device_id: u8) -> Result<String> {
		let mut cmd = [0u8; REPORT_SIZE];
		cmd[2] = device_id;
		cmd[3] = 6;
		cmd[4] = BL_PAGE;
		cmd[5] = BL_CMD_GET_VER;
		self.send(&cmd)?;
		std::thread::sleep(std::time::Duration::from_millis(100));
		let r = self.read()?;
		if r[1] == BL_STATUS_OK || r[1] == BL_STATUS_OK2 {
			self.hid_index = 0;
		} else if r[0] == BL_STATUS_OK || r[0] == BL_STATUS_OK2 {
			self.hid_index = 1;
		} else {
			return Ok("0.0.0.0".into());
		}
		let o = self.hid_index;
		Ok(format!(
			"{:x}.{:02x}.{:02x}.{:02x}",
			r[9 - o],
			r[10 - o],
			r[11 - o],
			r[12 - o]
		))
	}

	pub fn erase(&self, device_id: u8) -> Result<()> {
		let mut cmd = [0u8; REPORT_SIZE];
		cmd[2] = device_id;
		cmd[3] = 8;
		cmd[4] = BL_PAGE;
		cmd[5] = BL_CMD_ERASE;
		// erase takes a bit, poll till OK
		self.cmd_ack(&cmd, 30, 300, false)?;
		Ok(())
	}

	pub fn program(&self, device_id: u8, base: u32, data: &[u8], delays: &Delays) -> Result<()> {
		let total = data.chunks(32).len();
		let mut cache: i32 = 4096;
		for (i, chunk) in data.chunks(32).enumerate() {
			let addr = base + (i * 32) as u32;
			let mut cmd = [0u8; REPORT_SIZE];
			cmd[2] = device_id;
			cmd[3] = chunk.len() as u8 + 5;
			cmd[4] = BL_PAGE;
			cmd[5] = BL_CMD_PROGRAM;
			cmd[6] = chunk.len() as u8;
			cmd[7..11].copy_from_slice(&addr.to_be_bytes());
			// the web app XORs the whole payload part (as obfuscation i assume?), padding included
			for b in cmd[11..].iter_mut() {
				*b ^= XOR_KEY;
			}
			for (j, b) in chunk.iter().enumerate() {
				cmd[11 + j] = b ^ XOR_KEY;
			}
			let delay = if cache > 32 {
				cache -= 32;
				delays.program
			} else {
				cache = 4096;
				delays.program_4k
			};
			self.cmd_ack(&cmd, delay, 500, true)?;
			if i % 64 == 0 || i + 1 == total {
				eprint!("\rProgramming: {}%", (i + 1) * 100 / total);
			}
		}
		eprintln!();
		Ok(())
	}

	pub fn verify(&self, device_id: u8, base: u32, data: &[u8], delays: &Delays) -> Result<()> {
		let total = data.chunks(32).len();
		for (i, chunk) in data.chunks(32).enumerate() {
			let addr = base + (i * 32) as u32;
			let mut cmd = [0u8; REPORT_SIZE];
			cmd[2] = device_id;
			cmd[3] = 32;
			cmd[4] = BL_PAGE;
			cmd[5] = BL_CMD_VERIFY;
			cmd[6] = 32;
			cmd[7..11].copy_from_slice(&addr.to_be_bytes());
			let resp = self.cmd_ack(&cmd, delays.verify, 500, true)?;
			let o = self.hid_index;
			for (j, b) in chunk.iter().enumerate() {
				if resp[12 - o + j] ^ XOR_KEY != *b {
					bail!("verify mismatch at address {:#x}", addr + j as u32);
				}
			}
			if i % 64 == 0 || i + 1 == total {
				eprint!("\rVerifying: {}%", (i + 1) * 100 / total);
			}
		}
		eprintln!();
		Ok(())
	}

	pub fn enter_bl(&self, device_id: u8) {
		let mut cmd = [0u8; REPORT_SIZE];
		cmd[2] = device_id;
		cmd[3] = 1;
		cmd[6] = BL_PAGE;
		let _ = self.send(&cmd); // device MAY drop from bus mid-send
	}

	pub fn exit_bl(&self, device_id: u8) {
		let mut cmd = [0u8; REPORT_SIZE];
		cmd[2] = device_id;
		cmd[3] = 1;
		cmd[4] = BL_PAGE;
		cmd[5] = BL_CMD_EXIT;
		cmd[6] = BL_PAGE;
		let _ = self.send(&cmd); // device MAY drop from bus mid-send
	}
}

pub fn open_by_vid_pid(vid: u16, pid: u16) -> Result<hidapi::HidDevice> {
	let api = hidapi::HidApi::new()?;
	let mut paths: Vec<_> = api
		.device_list()
		.filter(|i| i.vendor_id() == vid && i.product_id() == pid)
		.collect();
	paths.sort_by_key(|i| !(i.usage_page() == 0xFFFF && i.usage() == 0));
	let present = !paths.is_empty();
	for info in paths {
		if let Ok(dev) = api.open_path(info.path()) {
			return Ok(dev);
		}
	}
	if present {
		bail!("device {vid:#06x}:{pid:#06x} present but could not be opened (permissions?)")
	}
	bail!("no device {vid:#06x}:{pid:#06x}")
}

/// wait for bl, report a `B` version.
pub fn wait_for_bl(vid: u16, pid: u16, device_id: u8) -> Result<Bl> {
	let mut last_err = None;
	for _ in 0..50 {
		match open_by_vid_pid(vid, pid) {
			Ok(dev) => {
				let mut bl = Bl::new(dev);
				for _ in 0..5 {
					if let Ok(v) = bl.fw_ver(device_id) {
						if v.to_lowercase().contains('b') {
							eprintln!("Bootloader mode: {v}");
							return Ok(bl);
						}
					}
					std::thread::sleep(std::time::Duration::from_millis(500));
				}
			}
			Err(e) => last_err = Some(e),
		}
		std::thread::sleep(std::time::Duration::from_millis(500));
	}
	if let Some(e) = last_err.filter(|e| e.to_string().contains("could not be opened")) {
		bail!(
			"{e}\nThe bootloader ({vid:04x}:{pid:04x}) needs a udev rule, e.g.:\n\
			 SUBSYSTEM==\"hidraw\", ATTRS{{idVendor}}==\"{vid:04x}\", ATTRS{{idProduct}}==\"{pid:04x}\", MODE=\"0666\", TAG+=\"uaccess\"\n\
			 (or just run this as root). The device returns to normal mode on its own."
		);
	}
	bail!("bootloader device {vid:#06x}:{pid:#06x} did not appear")
}

/// wait for device to come back after exitBL and report its version.
pub fn wait_for_app(vid: u16, pid: u16, device_id: u8) -> Result<String> {
	for _ in 0..50 {
		std::thread::sleep(std::time::Duration::from_millis(500));
		let Ok(dev) = open_by_vid_pid(vid, pid) else {
			continue;
		};
		let bl = Bl::new(dev);
		let mut cmd = [0u8; REPORT_SIZE];
		cmd[2] = device_id;
		cmd[3] = 0x10;
		cmd[5] = 0x81;
		if bl.send(&cmd).is_err() {
			continue;
		}
		std::thread::sleep(std::time::Duration::from_millis(100));
		let Ok(r) = bl.read() else { continue };
		let ver = if r[6] == 0x81 {
			format!("{}.{}.{}.{}", r[7], r[8], r[9], r[10])
		} else if r[5] == 0x81 {
			format!("{}.{}.{}.{}", r[6], r[7], r[8], r[9])
		} else {
			continue;
		};
		if ver != "0.0.0.0" {
			return Ok(ver);
		}
	}
	bail!("device did not come back after update")
}
