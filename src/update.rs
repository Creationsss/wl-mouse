use anyhow::{bail, Context, Result};

use crate::bootloader::{open_by_vid_pid, wait_for_app, wait_for_bl, Bl};
use crate::consts::*;
use crate::device::{Device, Snapshot};
use crate::types::{Delays, Model};

fn fetch(path: &str) -> Result<Vec<u8>> {
	let url = format!("{CONFIG_BASE}{}", path.replace(' ', "%20"));
	let mut resp = ureq::get(&url)
		.call()
		.with_context(|| format!("download failed: {url}"))?;
	Ok(resp.body_mut().read_to_vec()?)
}

fn fetch_models() -> Result<Vec<Model>> {
	let raw = fetch("env-models.json")?;
	let values: Vec<serde_json::Value> = serde_json::from_slice(&raw)?;
	Ok(values
		.into_iter()
		.filter(|v| v.get("ModelEN").is_some())
		.filter_map(|v| serde_json::from_value(v).ok())
		.collect())
}

/// "BEAST MINI PRO 8K Mouse_1_000_Chip_App_v01.00.02.12_20260411-bat(v506u6).hex" -> "1.0.2.12"
fn version_from_filename(name: &str) -> String {
	for part in name.split('_') {
		if part.contains('.') {
			return part
				.trim_start_matches('v')
				.split('.')
				.filter_map(|c| c.parse::<u64>().ok())
				.map(|n| n.to_string())
				.collect::<Vec<_>>()
				.join(".");
		}
	}
	String::new()
}

fn parse_intel_hex(text: &str) -> Result<(u32, Vec<u8>)> {
	let mut offset: u32 = 0;
	let mut records: Vec<(u32, Vec<u8>)> = Vec::new();

	for line in text.lines() {
		let line = line.trim();
		let Some(hex) = line.strip_prefix(':') else {
			continue;
		};
		let bytes: Vec<u8> = (0..hex.len() / 2)
			.map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16))
			.collect::<Result<_, _>>()
			.context("invalid hex digit in firmware file")?;
		if bytes.len() < 5 {
			bail!("truncated hex record");
		}
		let len = bytes[0] as usize;
		if bytes.len() != len + 5 {
			bail!("hex record length mismatch");
		}
		if bytes.iter().fold(0u8, |a, b| a.wrapping_add(*b)) != 0 {
			bail!("hex record checksum error");
		}
		let addr = ((bytes[1] as u32) << 8) | bytes[2] as u32;
		let data = &bytes[4..4 + len];
		match bytes[3] {
			0x00 => records.push((offset + addr, data.to_vec())),
			0x01 => break,
			0x02 => offset = (((data[0] as u32) << 8) | data[1] as u32) << 4,
			0x04 => offset = (((data[0] as u32) << 8) | data[1] as u32) << 16,
			_ => {}
		}
	}

	let base = records
		.iter()
		.map(|(a, _)| *a)
		.min()
		.context("no data records in hex file")?;
	let end = records
		.iter()
		.map(|(a, d)| *a + d.len() as u32)
		.max()
		.unwrap();
	let mut out = vec![0u8; (end - base) as usize];
	for (addr, data) in records {
		let i = (addr - base) as usize;
		out[i..i + data.len()].copy_from_slice(&data);
	}
	Ok((base, out))
}

fn parse_hex_u16(s: &str, what: &str) -> Result<u16> {
	u16::from_str_radix(s, 16).with_context(|| format!("bad {what} in env-models.json: {s:?}"))
}

struct Target {
	label: &'static str,
	bl_vid: u16,
	bl_pid: u16,
	files: Vec<String>,
	one_mcu_multi_hex: bool,
}

fn find_target(models: &[Model], pid: u16) -> Result<(&Model, Target)> {
	let hexpid = format!("{pid:04X}");
	let (model, mut target) = models
		.iter()
		.find_map(|m| {
			if m.pid_wired.eq_ignore_ascii_case(&hexpid) {
				Some((
					m,
					Target {
						label: "mouse",
						bl_vid: 0,
						bl_pid: 0,
						files: vec![m.device_fw_00.clone(), m.device_fw_01.clone()],
						one_mcu_multi_hex: false,
					},
				))
			} else if m.dongle_8k.eq_ignore_ascii_case(&hexpid) {
				Some((
					m,
					Target {
						label: "dongle",
						bl_vid: 0,
						bl_pid: 0,
						files: vec![m.recv_fw_00.clone(), m.recv_fw_01.clone()],
						one_mcu_multi_hex: m.one_mcu_multi_hex == "1",
					},
				))
			} else if m.dongle_1k.eq_ignore_ascii_case(&hexpid) {
				Some((
					m,
					Target {
						label: "1k-dongle",
						bl_vid: 0,
						bl_pid: 0,
						files: vec![],
						one_mcu_multi_hex: false,
					},
				))
			} else {
				None
			}
		})
		.with_context(|| format!("no firmware config for PID {pid:#06x}"))?;

	match target.label {
		"mouse" => {
			target.bl_vid = parse_hex_u16(&model.device_bl_vid, "DeviceBLVID")?;
			target.bl_pid = parse_hex_u16(&model.device_bl_pid, "DeviceBLPID")?;
		}
		"dongle" => {
			target.bl_vid = parse_hex_u16(&model.recv_bl_vid, "Receiver4K8KBLVID")?;
			target.bl_pid = parse_hex_u16(&model.recv_bl_pid, "Receiver4K8KBLPID")?;
		}
		_ => bail!("1K dongle updates are not supported"),
	}
	target.files.retain(|f| !f.is_empty());
	if target.files.is_empty() {
		bail!(
			"no firmware files listed for {} {}",
			model.model_en,
			target.label
		);
	}
	Ok((model, target))
}

pub fn cmd_update(path: Option<&str>, force: bool, yes: bool) -> Result<()> {
	eprintln!("Fetching {CONFIG_BASE}env-models.json ...");
	let models = fetch_models()?;

	let dev = Device::open(path)?;
	let pid = dev.pid();
	let (model, target) = find_target(&models, pid)?;

	let current = if target.label == "mouse" {
		dev.firmware_version()?
	} else {
		dev.dongle_firmware_version()?
	};
	let new_ver = version_from_filename(&target.files[0]);

	println!("Model:            {} ({})", model.model_en, target.label);
	println!("Current firmware: {current}");
	println!("New firmware:     {new_ver}");

	if current == new_ver && !force {
		println!("Already up to date (use --force to reflash).");
		return Ok(());
	}

	// download everything THEN update device
	let mut images: Vec<(u32, Vec<u8>)> = Vec::new();
	for f in &target.files {
		let url_path = format!("fwfiles/{}/{f}", model.fw_folder);
		eprintln!("Downloading {f} ...");
		let raw = fetch(&url_path)?;
		let text = String::from_utf8(raw).context("firmware file is not text")?;
		let (base, data) = parse_intel_hex(&text)?;
		eprintln!("  {} bytes at {base:#x}", data.len());
		images.push((base, data));
	}

	if !yes {
		eprint!(
			"\nAbout to flash {} firmware {new_ver}. Please do NOT unplug the device.\nContinue? [y/N] ",
			target.label
		);
		let mut line = String::new();
		std::io::stdin().read_line(&mut line)?;
		if !matches!(line.trim().to_lowercase().as_str(), "y" | "yes") {
			bail!("aborted");
		}
	}

	// the erase wipes the settings as well.
	let snapshot = if target.label == "mouse" {
		eprintln!("Backing up settings...");
		let s = Snapshot::take(&dev);
		match &s {
			Some(s) => eprintln!("Backed up {} profiles.", s.profile_count()),
			None => eprintln!("Warning: could not back up settings; defaults after update."),
		}
		s
	} else {
		None
	};

	// enter bootloader, device drops off bus then re-enumerates as BL pid
	dev.enter_bootloader(0);
	drop(dev);
	std::thread::sleep(std::time::Duration::from_millis(500));

	let mut bl = wait_for_bl(target.bl_vid, target.bl_pid, 0)?;
	std::thread::sleep(std::time::Duration::from_millis(1000));

	let (base, data) = &images[0];
	eprintln!("Flashing MCU 1 ({} bytes)...", data.len());
	bl.erase(0)?;
	bl.program(0, *base, data, &Delays::MCU1)?;
	bl.verify(0, *base, data, &Delays::MCU1)?;

	if images.len() > 1 {
		if !target.one_mcu_multi_hex {
			// leave BL, flash the second MCU with app-mode device
			bl.exit_bl(0);
			std::thread::sleep(std::time::Duration::from_millis(1000));
			wait_for_app(WL_VID, pid, 0)?;

			bl = Bl::new(open_by_vid_pid(WL_VID, pid)?);
			bl.enter_bl(1);
			std::thread::sleep(std::time::Duration::from_millis(500));
			let mut in_bl = false;
			for i in 0..50 {
				let v = bl.fw_ver(1).unwrap_or_default();
				if v.to_lowercase().contains('b') {
					eprintln!("MCU 2 bootloader: {v}");
					in_bl = true;
					break;
				}
				std::thread::sleep(std::time::Duration::from_millis(500));
				if i != 0 && i % 4 == 0 {
					bl.enter_bl(1);
					std::thread::sleep(std::time::Duration::from_millis(500));
				}
			}
			if !in_bl {
				bail!("MCU 2 did not enter bootloader mode");
			}
		}

		let (base, data) = &images[1];
		eprintln!("Flashing MCU 2 ({} bytes)...", data.len());
		bl.erase(1)?;
		bl.program(1, *base, data, &Delays::MCU2)?;
		bl.verify(1, *base, data, &Delays::MCU2)?;
		bl.exit_bl(1);
	} else {
		bl.exit_bl(0);
	}
	drop(bl);
	std::thread::sleep(std::time::Duration::from_millis(1000));

	let device_id = if images.len() > 1 { 1 } else { 0 };
	let ver = wait_for_app(WL_VID, pid, device_id)?;
	println!("Update successful. Firmware: {ver}");

	if let Some(s) = snapshot {
		std::thread::sleep(std::time::Duration::from_millis(1000));
		match s.restore() {
			Ok(()) => println!("Settings restored ({} profiles).", s.profile_count()),
			Err(e) => {
				eprintln!("Warning: settings restore failed ({e}); device is at factory defaults")
			}
		}
	}
	Ok(())
}
