mod cli;
mod consts;
mod device;
mod protocol;

use anyhow::{bail, Result};
use clap::Parser;

use cli::{Cli, Commands, DpiAction};
use device::Device;

fn main() -> Result<()> {
	let cli = Cli::parse();

	match cli.command {
		Commands::List => cmd_list(),
		Commands::Info => cmd_info(cli.device.as_deref()),
		Commands::Profile { id } => cmd_profile(cli.device.as_deref(), id),
		Commands::Dpi { action } => cmd_dpi(cli.device.as_deref(), action),
		Commands::PollingRate { rate } => cmd_polling_rate(cli.device.as_deref(), rate),
		Commands::Lod { value } => cmd_lod(cli.device.as_deref(), value),
		Commands::Debounce { ms } => cmd_debounce(cli.device.as_deref(), ms),
		Commands::AngleSnap { value } => cmd_toggle(
			cli.device.as_deref(),
			"Angle snap",
			value,
			|d, p| d.angle_snap(p),
			|d, p, v| d.set_angle_snap(p, v),
		),
		Commands::MotionSync { value } => cmd_toggle(
			cli.device.as_deref(),
			"Motion sync",
			value,
			|d, p| d.motion_sync(p),
			|d, p, v| d.set_motion_sync(p, v),
		),
		Commands::AngleTune { value } => cmd_angle_tune(cli.device.as_deref(), value),
		Commands::RippleControl { value } => cmd_toggle(
			cli.device.as_deref(),
			"Ripple control",
			value,
			|d, p| d.ripple_control(p),
			|d, p, v| d.set_ripple_control(p, v),
		),
		Commands::SleepTime { minutes } => cmd_sleep_time(cli.device.as_deref(), minutes),
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

fn cmd_list() -> Result<()> {
	let devices = device::list_devices()?;
	if devices.is_empty() {
		println!("No WLmouse devices found.");
	} else {
		for (name, pid, path) in &devices {
			println!("{name} (PID:{pid:#06x}) at {path}");
		}
	}
	Ok(())
}

fn cmd_info(path: Option<&str>) -> Result<()> {
	let dev = Device::open(path)?;
	println!("WLmouse {}", dev.name);

	let fw = dev.firmware_version()?;
	println!("  Mouse firmware:   {fw}");

	if let Ok(dfw) = dev.dongle_firmware_version() {
		println!("  Dongle firmware:  {dfw}");
	}

	if let Ok((percent, charging)) = dev.battery() {
		let status = if charging { " (charging)" } else { "" };
		println!("  Battery:          {percent}%{status}");
	}

	if let Ok(sn) = dev.serial_number() {
		if !sn.is_empty() {
			println!("  Serial:           {sn}");
		}
	}

	let profile = dev.active_profile()?;
	println!("  Active profile:   {profile}");

	Ok(())
}

fn cmd_profile(path: Option<&str>, id: Option<u8>) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = id.unwrap_or(dev.active_profile()?);

	println!("Profile {profile}:");

	let rate = dev.polling_rate(profile)?;
	println!("  Polling rate:  {rate} Hz");

	let (active, stages) = dev.dpi_stages(profile, 6)?;
	println!("  DPI stages:");
	for (i, (x, y)) in stages.iter().enumerate() {
		let marker = if i as u8 == active { " *" } else { "" };
		if x == y {
			println!("    {}: {x}{marker}", i + 1);
		} else {
			println!("    {}: X={x} Y={y}{marker}", i + 1);
		}
	}

	let lod = dev.lod(profile)?;
	println!("  LOD:           {lod}mm");

	let debounce = dev.debounce(profile)?;
	println!("  Debounce:      {debounce}ms");

	if let Ok(snap) = dev.angle_snap(profile) {
		println!("  Angle snap:    {}", if snap { "on" } else { "off" });
	}

	if let Ok(sync) = dev.motion_sync(profile) {
		println!("  Motion sync:   {}", if sync { "on" } else { "off" });
	}

	if let Ok(tune) = dev.angle_tune(profile) {
		println!("  Angle tune:    {tune}");
	}

	if let Ok(ripple) = dev.ripple_control(profile) {
		println!("  Ripple ctrl:   {}", if ripple { "on" } else { "off" });
	}

	if let Ok(sleep) = dev.sleep_time(profile) {
		if sleep == 0 || sleep >= 65280 {
			println!("  Sleep time:    off");
		} else {
			println!("  Sleep time:    {}min", sleep / 60);
		}
	}

	Ok(())
}

fn cmd_dpi(path: Option<&str>, action: Option<DpiAction>) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match action {
		None => {
			let (active, stages) = dev.dpi_stages(profile, 6)?;
			for (i, (x, y)) in stages.iter().enumerate() {
				let marker = if i as u8 == active { " *" } else { "" };
				if x == y {
					println!("Stage {}: {x}{marker}", i + 1);
				} else {
					println!("Stage {}: X={x} Y={y}{marker}", i + 1);
				}
			}
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
			stages[idx] = (dpi, y_dpi.unwrap_or(dpi));
			dev.set_dpi_stages(profile, &stages)?;
			println!("Stage {stage} set to {dpi} DPI");
		}
		Some(DpiAction::Active { stage }) => {
			if !(1..=6).contains(&stage) {
				bail!("stage must be 1-6");
			}
			dev.set_active_dpi(profile, stage)?;
			println!("Active DPI stage set to {stage}");
		}
	}
	Ok(())
}

fn cmd_polling_rate(path: Option<&str>, rate: Option<u16>) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match rate {
		None => {
			let rate = dev.polling_rate(profile)?;
			println!("{rate} Hz");
		}
		Some(r) => {
			if !matches!(r, 125 | 250 | 500 | 1000 | 2000 | 4000 | 8000) {
				bail!("invalid polling rate: {r} (valid: 125/250/500/1000/2000/4000/8000)");
			}
			dev.set_polling_rate(profile, r)?;
			println!("Polling rate set to {r} Hz");
		}
	}
	Ok(())
}

fn cmd_lod(path: Option<&str>, value: Option<f32>) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match value {
		None => {
			let lod = dev.lod(profile)?;
			println!("{lod}mm");
		}
		Some(v) => {
			let valid = [0.7_f32, 1.0, 2.0];
			if !valid.iter().any(|&x| (x - v).abs() < 0.01) {
				bail!("invalid LOD: {v} (valid: 0.7, 1, 2)");
			}
			dev.set_lod(profile, v)?;
			println!("LOD set to {v}mm");
		}
	}
	Ok(())
}

fn cmd_debounce(path: Option<&str>, ms: Option<u8>) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match ms {
		None => {
			let d = dev.debounce(profile)?;
			println!("{d}ms");
		}
		Some(v) => {
			dev.set_debounce(profile, v)?;
			println!("Debounce set to {v}ms");
		}
	}
	Ok(())
}

fn cmd_toggle(
	path: Option<&str>,
	label: &str,
	value: Option<String>,
	getter: impl Fn(&Device, u8) -> Result<bool>,
	setter: impl Fn(&Device, u8, bool) -> Result<()>,
) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match value {
		None => {
			let v = getter(&dev, profile)?;
			println!("{}", if v { "on" } else { "off" });
		}
		Some(s) => {
			let enabled = parse_bool(&s)?;
			setter(&dev, profile, enabled)?;
			println!("{label} set to {}", if enabled { "on" } else { "off" });
		}
	}
	Ok(())
}

fn cmd_angle_tune(path: Option<&str>, value: Option<i8>) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match value {
		None => {
			let v = dev.angle_tune(profile)?;
			println!("{v}");
		}
		Some(v) => {
			dev.set_angle_tune(profile, v)?;
			println!("Angle tune set to {v}");
		}
	}
	Ok(())
}

fn cmd_sleep_time(path: Option<&str>, minutes: Option<u16>) -> Result<()> {
	let dev = Device::open(path)?;
	let profile = dev.active_profile()?;

	match minutes {
		None => {
			let secs = dev.sleep_time(profile)?;
			if secs == 0 || secs >= 65280 {
				println!("off");
			} else {
				println!("{}min", secs / 60);
			}
		}
		Some(0) => {
			dev.set_sleep_time(profile, 65535)?;
			println!("Sleep disabled");
		}
		Some(m @ 1..=30) => {
			dev.set_sleep_time(profile, m * 60)?;
			println!("Sleep time set to {m}min");
		}
		Some(m) => {
			bail!("invalid sleep time: {m} (valid: 0=off, 1-30 minutes)");
		}
	}
	Ok(())
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
