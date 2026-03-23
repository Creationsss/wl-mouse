use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Result};

fn socket_dir() -> Result<PathBuf> {
	let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").map_err(|_| {
		anyhow::anyhow!("--focus requires Hyprland (HYPRLAND_INSTANCE_SIGNATURE not set)")
	})?;
	let runtime =
		std::env::var("XDG_RUNTIME_DIR").map_err(|_| anyhow::anyhow!("XDG_RUNTIME_DIR not set"))?;
	Ok(PathBuf::from(runtime).join("hypr").join(sig))
}

pub fn find_window_by_pid(pid: u32) -> Result<String> {
	let output = std::process::Command::new("hyprctl")
		.args(["clients", "-j"])
		.output()?;
	let clients: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)?;
	for client in &clients {
		if client["pid"].as_u64() == Some(pid as u64) {
			if let Some(addr) = client["address"].as_str() {
				return Ok(addr.trim_start_matches("0x").to_string());
			}
		}
	}
	bail!("window not found for PID {pid}")
}

pub struct FocusMonitor {
	reader: BufReader<UnixStream>,
	window_addr: String,
	pub focused: bool,
}

impl FocusMonitor {
	pub fn new(window_addr: String) -> Result<Self> {
		let path = socket_dir()?.join(".socket2.sock");
		let socket = UnixStream::connect(&path)?;
		socket.set_read_timeout(Some(Duration::from_millis(100)))?;

		Ok(Self {
			reader: BufReader::new(socket),
			window_addr,
			focused: false,
		})
	}

	pub fn poll(&mut self) -> Option<bool> {
		let mut line = String::new();
		match self.reader.read_line(&mut line) {
			Ok(n) if n > 0 => {
				let line = line.trim();
				if let Some(addr) = line.strip_prefix("activewindowv2>>") {
					let now_focused = addr == self.window_addr;
					if now_focused != self.focused {
						self.focused = now_focused;
						return Some(now_focused);
					}
				}
				None
			}
			_ => None,
		}
	}
}
