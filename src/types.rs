use serde::{Deserialize, Serialize};

use crate::consts::*;

#[derive(Serialize)]
pub struct DeviceListItem {
	pub name: String,
	pub pid: u16,
	pub path: String,
}

#[derive(Serialize)]
pub struct DeviceInfo {
	pub name: String,
	pub firmware: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub dongle_firmware: Option<String>,
	pub battery_percent: Option<u8>,
	pub charging: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub serial_number: Option<String>,
	pub active_profile: u8,
}

#[derive(Serialize)]
pub struct DpiStage {
	pub x: u16,
	pub y: u16,
	pub active: bool,
}

#[derive(Serialize)]
pub struct ProfileInfo {
	pub id: u8,
	pub polling_rate_hz: u16,
	pub dpi_stages: Vec<DpiStage>,
	pub lod_mm: f32,
	pub debounce_ms: u8,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub angle_snap: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub motion_sync: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub angle_tune: Option<i8>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub ripple_control: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub high_speed: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub turbo: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub sleep_time_seconds: Option<u16>,
}

pub fn dpi_stages_from_raw(active: u8, stages: &[(u16, u16)]) -> Vec<DpiStage> {
	stages
		.iter()
		.enumerate()
		.map(|(i, (x, y))| DpiStage {
			x: *x,
			y: *y,
			active: i as u8 == active,
		})
		.collect()
}

pub fn normalize_sleep_time(raw: u16) -> u16 {
	if raw == SLEEP_OFF || raw >= SLEEP_DISABLED {
		0
	} else {
		raw
	}
}

/// bl packet pacing to match web (ms).
pub struct Delays {
	pub program: u64,
	pub program_4k: u64,
	pub verify: u64,
}

impl Delays {
	pub const MCU1: Delays = Delays {
		program: 1,
		program_4k: 1,
		verify: 1,
	};
	pub const MCU2: Delays = Delays {
		program: 4,
		program_4k: 5,
		verify: 5,
	};
}

/// env-models.json
#[derive(Deserialize, Default)]
#[serde(default)]
pub struct Model {
	#[serde(rename = "ModelEN")]
	pub model_en: String,
	#[serde(rename = "FWFolder")]
	pub fw_folder: String,
	#[serde(rename = "PIDWired")]
	pub pid_wired: String,
	#[serde(rename = "DeviceBLVID")]
	pub device_bl_vid: String,
	#[serde(rename = "DeviceBLPID")]
	pub device_bl_pid: String,
	#[serde(rename = "DeviceFWFile_00")]
	pub device_fw_00: String,
	#[serde(rename = "DeviceFWFile_01")]
	pub device_fw_01: String,
	#[serde(rename = "ReceiverOneMcuMultiHex")]
	pub one_mcu_multi_hex: String,
	#[serde(rename = "_8KDongle")]
	pub dongle_8k: String,
	#[serde(rename = "_1KDongle")]
	pub dongle_1k: String,
	#[serde(rename = "Receiver4K8KBLVID")]
	pub recv_bl_vid: String,
	#[serde(rename = "Receiver4K8KBLPID")]
	pub recv_bl_pid: String,
	#[serde(rename = "Receiver4K8KFWFile_00")]
	pub recv_fw_00: String,
	#[serde(rename = "Receiver4K8KFWFile_01")]
	pub recv_fw_01: String,
}
