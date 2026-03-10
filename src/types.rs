use serde::Serialize;

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
