use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "wl-mouse", about = "CLI for WLmouse gaming mice")]
pub struct Cli {
	#[arg(short, long, help = "HID device path (auto-detects if not specified)")]
	pub device: Option<String>,

	#[command(subcommand)]
	pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
	#[command(about = "Show device info (firmware, battery, serial)")]
	Info,

	#[command(about = "List connected WLmouse devices")]
	List,

	#[command(about = "Show current profile settings")]
	Profile {
		#[arg(short, long, help = "Profile ID to read (default: active)")]
		id: Option<u8>,
	},

	#[command(about = "Get or set DPI stages")]
	Dpi {
		#[command(subcommand)]
		action: Option<DpiAction>,
	},

	#[command(about = "Get or set polling rate")]
	PollingRate {
		#[arg(help = "Rate in Hz (125/250/500/1000/2000/4000/8000)")]
		rate: Option<u16>,
	},

	#[command(about = "Get or set lift-off distance")]
	Lod {
		#[arg(help = "LOD value (0.7, 1, 2)")]
		value: Option<f32>,
	},

	#[command(about = "Get or set debounce time")]
	Debounce {
		#[arg(help = "Debounce time in ms")]
		ms: Option<u8>,
	},

	#[command(about = "Get or set angle snapping")]
	AngleSnap {
		#[arg(help = "Enable/disable (on/off)")]
		value: Option<String>,
	},

	#[command(about = "Get or set motion sync")]
	MotionSync {
		#[arg(help = "Enable/disable (on/off)")]
		value: Option<String>,
	},

	#[command(about = "Get or set angle tuning")]
	AngleTune {
		#[arg(help = "Angle tune value (-30 to 30)")]
		value: Option<i8>,
	},

	#[command(about = "Get or set ripple control")]
	RippleControl {
		#[arg(help = "Enable/disable (on/off)")]
		value: Option<String>,
	},

	#[command(about = "Get or set sleep time")]
	SleepTime {
		#[arg(help = "Sleep time in minutes (0 = never)")]
		minutes: Option<u16>,
	},

	#[command(about = "Apply settings, run a command, then restore")]
	Wrap {
		#[arg(short, long, help = "Polling rate in Hz (default: 8000)")]
		rate: Option<u16>,
		#[arg(long, help = "DPI value")]
		dpi: Option<u16>,
		#[arg(long, help = "LOD value (0.7, 1, 2)")]
		lod: Option<f32>,
		#[arg(long, help = "Debounce time in ms")]
		debounce: Option<u8>,
		#[arg(trailing_var_arg = true, required = true)]
		command: Vec<String>,
	},

	#[command(about = "Factory reset the device")]
	Reset,
}

#[derive(Subcommand)]
pub enum DpiAction {
	#[command(about = "Set DPI for a stage (e.g. dpi set 1 800)")]
	Set {
		#[arg(help = "Stage number (1-6)")]
		stage: u8,
		#[arg(help = "DPI value")]
		dpi: u16,
		#[arg(short, long, help = "Also set Y-axis DPI separately")]
		y_dpi: Option<u16>,
	},
	#[command(about = "Set the active DPI stage")]
	Active {
		#[arg(help = "Stage number (1-6)")]
		stage: u8,
	},
}
