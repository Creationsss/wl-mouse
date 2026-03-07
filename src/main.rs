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
