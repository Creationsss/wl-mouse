pub const WL_VID: u16 = 0x36A7;

pub const REPORT_ID: u8 = 0;
pub const REPORT_SIZE: usize = 64;
pub const RESPONSE_OK: u8 = 0xA1;
pub const RESPONSE_SLEEPING: u8 = 0xA0;
pub const MAX_RETRIES: u8 = 10;

pub const KNOWN_PIDS: &[(u16, &str)] = &[
	(0xA864, "HUAN (wired)"),
	(0xA863, "HUAN (dongle)"),
	(0xA867, "BEAST MIAO (wired)"),
	(0xA866, "BEAST MIAO (dongle)"),
	(0xA882, "WLmouse (1K dongle)"),
	(0xA873, "STRIDER (wired)"),
	(0xA872, "STRIDER (dongle)"),
	(0xA875, "YING (wired)"),
	(0xA874, "YING (dongle)"),
	(0xA879, "SWORD X (wired)"),
	(0xA878, "SWORD X (dongle)"),
	(0xA886, "BEAST MINI (wired)"),
	(0xA885, "BEAST MINI (dongle)"),
	(0xA869, "BEAST MINI PRO (wired)"),
	(0xA868, "BEAST MINI PRO (dongle)"),
	(0xA881, "BEAST MAX (wired)"),
	(0xA880, "BEAST MAX (dongle)"),
	(0xA884, "BEAST X (wired)"),
	(0xA883, "BEAST X (dongle)"),
	(0xA871, "BEAST X PRO (wired)"),
	(0xA870, "BEAST X PRO (dongle)"),
];

pub const SLEEP_OFF: u16 = 0x0000;
pub const SLEEP_DISABLED: u16 = 0xFF00; // 65280 (turbo mode)
pub const SLEEP_MAX_VAL: u16 = 0xFFFF; // 65535
