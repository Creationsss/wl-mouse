mod cli;
mod consts;
mod device;
mod protocol;
mod types;

use anyhow::{bail, Result};
use clap::Parser;
use serde::Serialize;

use cli::{Cli, Commands, DpiAction};
use consts::*;
use device::Device;
use types::*;

fn print_output<T: Serialize>(data: T, json: bool, plain_printer: impl FnOnce(T)) -> Result<()> {
	if json {
		println!("{}", serde_json::to_string_pretty(&data)?);
	} else {
		plain_printer(data);
	}
	Ok(())
}

fn main() -> Result<()> {
	let cli = Cli::parse();

	match cli.command {
		Commands::List => cmd_list(cli.json),
		Commands::Info => cmd_info(cli.device.as_deref(), cli.json),
		Commands::Profile { id } => cmd_profile(cli.device.as_deref(), id, cli.json),
		Commands::Dpi { action } => cmd_dpi(cli.device.as_deref(), action, cli.json),
		Commands::PollingRate { rate } => cmd_polling_rate(cli.device.as_deref(), rate, cli.json),
		Commands::Lod { value } => cmd_lod(cli.device.as_deref(), value, cli.json),
		Commands::Debounce { ms } => cmd_debounce(cli.device.as_deref(), ms, cli.json),
		Commands::AngleSnap { value } => cmd_toggle(
			cli.device.as_deref(),
			"Angle snap",
			value,
			cli.json,
			|d, p| d.angle_snap(p),
			|d, p, v| d.set_angle_snap(p, v),
		),
		Commands::MotionSync { value } => cmd_toggle(
			cli.device.as_deref(),
			"Motion sync",
			value,
			cli.json,
			|d, p| d.motion_sync(p),
			|d, p, v| d.set_motion_sync(p, v),
		),
		Commands::AngleTune { value } => cmd_angle_tune(cli.device.as_deref(), value, cli.json),
		Commands::RippleControl { value } => cmd_toggle(
			cli.device.as_deref(),
			"Ripple control",
			value,
			cli.json,
			|d, p| d.ripple_control(p),
			|d, p, v| d.set_ripple_control(p, v),
		),
		Commands::SleepTime { minutes } => cmd_sleep_time(cli.device.as_deref(), minutes, cli.json),
		Commands::Wrap {
			rate,
			dpi,
			lod,
			debounce,
			command,
		} => cmd_wrap(cli.device.as_deref(), rate, dpi, lod, debounce, &command),
		Commands::Reset => cmd_reset(cli.device.as_deref()),
	}
}

fn cmd_list(json: bool) -> Result<()> {
	let devices = device::list_devices()?;
	let list: Vec<DeviceListItem> = devices
		.into_iter()
		.map(|(name, pid, path)| DeviceListItem { name, pid, path })
		.collect();

	print_output(list, json, |items| {
		if items.is_empty() {
			println!("No WLmouse devices found.");
		} else {
			for item in items {
				println!("{} (PID:{:#06x}) at {}", item.name, item.pid, item.path);
			}
		}
	})
}

fn cmd_info(path: Option<&str>, json: bool) -> Result<()> {
	let dev = Device::open(path)?;

	let fw = dev.firmware_version()?;
	let dfw = dev.dongle_firmware_version().ok();
	let battery = dev.battery().ok();
	let sn = dev.serial_number().ok();
	let profile = dev.active_profile()?;

	let info = DeviceInfo {
		name: dev.name,
		firmware: fw,
		dongle_firmware: dfw,
		battery_percent: battery.map(|b| b.0),
		charging: battery.map(|b| b.1),
		serial_number: sn,
		active_profile: profile,
	};

	print_output(info, json, |info| {
		println!("WLmouse {}", info.name);
		println!("  Mouse firmware:   {}", info.firmware);

		if let Some(dfw) = info.dongle_firmware {
			println!("  Dongle firmware:  {dfw}");
		}

		if let (Some(percent), Some(charging)) = (info.battery_percent, info.charging) {
			let status = if charging { " (charging)" } else { "" };
			println!("  Battery:          {percent}%{status}");
		}

		if let Some(sn) = info.serial_number {
			if !sn.is_empty() {
				println!("  Serial:           {sn}");
			}
		}

		println!("  Active profile:   {}", info.active_profile);
	})
}

fn cmd_profile(path: Option<&str>, id: Option<u8>, json: bool) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = id.unwrap_or(dev.active_profile()?);

	let rate = dev.polling_rate(profile)?;
	let (active_stage, stages) = dev.dpi_stages(profile, 6)?;
	let lod = dev.lod(profile)?;
	let debounce = dev.debounce(profile)?;
	let snap = dev.angle_snap(profile).ok();
	let sync = dev.motion_sync(profile).ok();
	let tune = dev.angle_tune(profile).ok();
	let ripple = dev.ripple_control(profile).ok();
	let sleep = dev.sleep_time(profile).ok();

	let info = ProfileInfo {
		id: profile,
		polling_rate_hz: rate,
		dpi_stages: dpi_stages_from_raw(active_stage, &stages),
		lod_mm: lod,
		debounce_ms: debounce,
		angle_snap: snap,
		motion_sync: sync,
		angle_tune: tune,
		ripple_control: ripple,
		sleep_time_seconds: sleep.map(normalize_sleep_time),
	};

	print_output(info, json, |info| {
		println!("Profile {}:", info.id);
		println!("  Polling rate:  {} Hz", info.polling_rate_hz);

		println!("  DPI stages:");
		for (i, stage) in info.dpi_stages.iter().enumerate() {
			let marker = if stage.active { " *" } else { "" };
			if stage.x == stage.y {
				println!("    {}: {}{}", i + 1, stage.x, marker);
			} else {
				println!("    {}: X={} Y={}{}", i + 1, stage.x, stage.y, marker);
			}
		}

		println!("  LOD:           {}mm", info.lod_mm);
		println!("  Debounce:      {}ms", info.debounce_ms);

		if let Some(snap) = info.angle_snap {
			println!("  Angle snap:    {}", if snap { "on" } else { "off" });
		}

		if let Some(sync) = info.motion_sync {
			println!("  Motion sync:   {}", if sync { "on" } else { "off" });
		}

		if let Some(tune) = info.angle_tune {
			println!("  Angle tune:    {tune}");
		}

		if let Some(ripple) = info.ripple_control {
			println!("  Ripple ctrl:   {}", if ripple { "on" } else { "off" });
		}

		if let Some(sleep) = info.sleep_time_seconds {
			if sleep == 0 {
				println!("  Sleep time:    off");
			} else {
				println!("  Sleep time:    {}min", sleep / 60);
			}
		}
	})
}

fn cmd_dpi(path: Option<&str>, action: Option<DpiAction>, json: bool) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match action {
		None => {
			let (active, stages) = dev.dpi_stages(profile, 6)?;
			let list = dpi_stages_from_raw(active, &stages);

			print_output(list, json, |stages| {
				for (i, stage) in stages.iter().enumerate() {
					let marker = if stage.active { " *" } else { "" };
					if stage.x == stage.y {
						println!("Stage {}: {}{}", i + 1, stage.x, marker);
					} else {
						println!("Stage {}: X={} Y={}{}", i + 1, stage.x, stage.y, marker);
					}
				}
			})
		}
		Some(DpiAction::Set { stage, dpi, y_dpi }) => {
			if !(1..=6).contains(&stage) {
				bail!("stage must be 1-6");
			}
			let (_, mut stages) = dev.dpi_stages(profile, 6)?;
			let idx = (stage - 1) as usize;
			if idx >= stages.len() {
				bail!(
					"stage {stage} doesn't exist (device has {} stages)",
					stages.len()
				);
			}
			let y = y_dpi.unwrap_or(dpi);
			stages[idx] = (dpi, y);
			dev.set_dpi_stages(profile, &stages)?;

			print_output(
				serde_json::json!({ "stage": stage, "dpi": dpi, "y_dpi": y }),
				json,
				|_| println!("Stage {stage} set to {dpi} DPI"),
			)
		}
		Some(DpiAction::Active { stage }) => {
			if !(1..=6).contains(&stage) {
				bail!("stage must be 1-6");
			}
			dev.set_active_dpi(profile, stage)?;

			print_output(serde_json::json!({ "active_stage": stage }), json, |_| {
				println!("Active DPI stage set to {stage}")
			})
		}
	}
}

fn cmd_polling_rate(path: Option<&str>, rate: Option<u16>, json: bool) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match rate {
		None => {
			let rate = dev.polling_rate(profile)?;
			print_output(serde_json::json!({ "polling_rate_hz": rate }), json, |_| {
				println!("{rate} Hz")
			})
		}
		Some(r) => {
			if !matches!(r, 125 | 250 | 500 | 1000 | 2000 | 4000 | 8000) {
				bail!("invalid polling rate: {r} (valid: 125/250/500/1000/2000/4000/8000)");
			}
			dev.set_polling_rate(profile, r)?;
			print_output(serde_json::json!({ "polling_rate_hz": r }), json, |_| {
				println!("Polling rate set to {r} Hz")
			})
		}
	}
}

fn cmd_lod(path: Option<&str>, value: Option<f32>, json: bool) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match value {
		None => {
			let lod = dev.lod(profile)?;
			print_output(serde_json::json!({ "lod_mm": lod }), json, |_| {
				println!("{lod}mm")
			})
		}
		Some(v) => {
			let valid = [0.7_f32, 1.0, 2.0];
			if !valid.iter().any(|&x| (x - v).abs() < 0.01) {
				bail!("invalid LOD: {v} (valid: 0.7, 1, 2)");
			}
			dev.set_lod(profile, v)?;
			print_output(serde_json::json!({ "lod_mm": v }), json, |_| {
				println!("LOD set to {v}mm")
			})
		}
	}
}

fn cmd_debounce(path: Option<&str>, ms: Option<u8>, json: bool) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match ms {
		None => {
			let d = dev.debounce(profile)?;
			print_output(serde_json::json!({ "debounce_ms": d }), json, |_| {
				println!("{d}ms")
			})
		}
		Some(v) => {
			dev.set_debounce(profile, v)?;
			print_output(serde_json::json!({ "debounce_ms": v }), json, |_| {
				println!("Debounce set to {v}ms")
			})
		}
	}
}

fn cmd_toggle(
	path: Option<&str>,
	label: &str,
	value: Option<String>,
	json: bool,
	getter: impl Fn(&Device, u8) -> Result<bool>,
	setter: impl Fn(&Device, u8, bool) -> Result<()>,
) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	let field_name = label.to_lowercase().replace(' ', "_");

	match value {
		None => {
			let v = getter(&dev, profile)?;
			print_output(serde_json::json!({ field_name: v }), json, |_| {
				println!("{}", if v { "on" } else { "off" })
			})
		}
		Some(s) => {
			let enabled = parse_bool(&s)?;
			setter(&dev, profile, enabled)?;
			print_output(serde_json::json!({ field_name: enabled }), json, |_| {
				println!("{label} set to {}", if enabled { "on" } else { "off" })
			})
		}
	}
}

fn cmd_angle_tune(path: Option<&str>, value: Option<i8>, json: bool) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match value {
		None => {
			let v = dev.angle_tune(profile)?;
			print_output(serde_json::json!({ "angle_tune": v }), json, |_| {
				println!("{v}")
			})
		}
		Some(v) => {
			dev.set_angle_tune(profile, v)?;
			print_output(serde_json::json!({ "angle_tune": v }), json, |_| {
				println!("Angle tune set to {v}")
			})
		}
	}
}

fn cmd_sleep_time(path: Option<&str>, minutes: Option<u16>, json: bool) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match minutes {
		None => {
			let secs = dev.sleep_time(profile)?;
			let val = normalize_sleep_time(secs);
			print_output(
				serde_json::json!({ "sleep_time_seconds": val }),
				json,
				|_| {
					if val == 0 {
						println!("off");
					} else {
						println!("{}min", val / 60);
					}
				},
			)
		}
		Some(0) => {
			dev.set_sleep_time(profile, SLEEP_MAX_VAL)?;
			print_output(serde_json::json!({ "sleep_time_seconds": 0 }), json, |_| {
				println!("Sleep disabled")
			})
		}
		Some(m @ 1..=30) => {
			let secs = m * 60;
			dev.set_sleep_time(profile, secs)?;
			print_output(
				serde_json::json!({ "sleep_time_seconds": secs }),
				json,
				|_| println!("Sleep time set to {m}min"),
			)
		}
		Some(m) => {
			bail!("invalid sleep time: {m} (valid: 0=off, 1-30 minutes)");
		}
	}
}

fn cmd_wrap(
	path: Option<&str>,
	rate: Option<u16>,
	dpi: Option<u16>,
	lod: Option<f32>,
	debounce: Option<u8>,
	command: &[String],
) -> Result<()> {
	let rate = rate.or(if dpi.is_none() && lod.is_none() && debounce.is_none() {
		Some(8000)
	} else {
		None
	});

	if let Some(r) = rate {
		if !matches!(r, 125 | 250 | 500 | 1000 | 2000 | 4000 | 8000) {
			bail!("invalid polling rate: {r} (valid: 125/250/500/1000/2000/4000/8000)");
		}
	}
	if let Some(v) = lod {
		let valid = [0.7_f32, 1.0, 2.0];
		if !valid.iter().any(|&x| (x - v).abs() < 0.01) {
			bail!("invalid LOD: {v} (valid: 0.7, 1, 2)");
		}
	}

	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	let orig_rate = rate.map(|_| dev.polling_rate(profile)).transpose()?;
	let orig_dpi = dpi.map(|_| dev.dpi_stages(profile, 6)).transpose()?;
	let orig_lod = lod.map(|_| dev.lod(profile)).transpose()?;
	let orig_debounce = debounce.map(|_| dev.debounce(profile)).transpose()?;

	if let (Some(r), Some(orig)) = (rate, orig_rate) {
		if orig != r {
			dev.set_polling_rate(profile, r)?;
			eprintln!("Polling rate: {orig} -> {r} Hz");
		}
	}
	let mut dpi_changed = false;
	if let (Some(d), Some((active, ref stages))) = (dpi, &orig_dpi) {
		let current = stages.get(*active as usize).map(|s| s.0).unwrap_or(0);
		if current != d {
			let new_stages: Vec<(u16, u16)> = stages.iter().map(|_| (d, d)).collect();
			dev.set_dpi_stages(profile, &new_stages)?;
			eprintln!("DPI: {current} -> {d}");
			dpi_changed = true;
		}
	}
	if let (Some(v), Some(orig)) = (lod, orig_lod) {
		if (orig - v).abs() > 0.01 {
			dev.set_lod(profile, v)?;
			eprintln!("LOD: {orig}mm -> {v}mm");
		}
	}
	if let (Some(d), Some(orig)) = (debounce, orig_debounce) {
		if orig != d {
			dev.set_debounce(profile, d)?;
			eprintln!("Debounce: {orig}ms -> {d}ms");
		}
	}

	let status = std::process::Command::new(&command[0])
		.args(&command[1..])
		.status();

	if let (Some(orig), Some(r)) = (orig_rate, rate) {
		if orig != r {
			dev.set_polling_rate(profile, orig)?;
			eprintln!("Polling rate restored: {orig} Hz");
		}
	}
	if let (Some((_, ref stages)), true) = (&orig_dpi, dpi_changed) {
		dev.set_dpi_stages(profile, stages)?;
		eprintln!("DPI stages restored");
	}
	if let (Some(orig), Some(v)) = (orig_lod, lod) {
		if (orig - v).abs() > 0.01 {
			dev.set_lod(profile, orig)?;
			eprintln!("LOD restored: {orig}mm");
		}
	}
	if let (Some(orig), Some(d)) = (orig_debounce, debounce) {
		if orig != d {
			dev.set_debounce(profile, orig)?;
			eprintln!("Debounce restored: {orig}ms");
		}
	}

	let code = status?.code().unwrap_or(1);
	std::process::exit(code);
}

fn cmd_reset(path: Option<&str>) -> Result<()> {
	let dev = Device::open(path)?;
	dev.factory_reset()?;
	println!("Factory reset sent.");
	Ok(())
}

fn parse_bool(s: &str) -> Result<bool> {
	match s.to_lowercase().as_str() {
		"on" | "true" | "1" | "yes" | "enable" => Ok(true),
		"off" | "false" | "0" | "no" | "disable" => Ok(false),
		_ => bail!("expected on/off, got '{s}'"),
	}
}
