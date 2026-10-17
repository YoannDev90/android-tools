//! Shared `adb` helpers for the android-tools workspace.
//!
//! Generic plumbing only: device picking, shell/exec-out wrappers with
//! timeouts. Domain logic (logcat streaming, UI hierarchy, screenshots)
//! stays in the tools that own it.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

/// Pick a serial: explicit flag, or the single connected device.
pub fn pick_serial(explicit: Option<&str>) -> Result<String> {
	if let Some(s) = explicit {
		return Ok(s.to_string());
	}
	let out = Command::new("adb")
		.args(["devices"])
		.output()
		.context("adb not found in PATH — install platform-tools")?;
	let mut devices = parse_devices(&String::from_utf8_lossy(&out.stdout));
	match devices.len() {
		0 => bail!("no adb device connected (or unauthorized)"),
		1 => Ok(devices.remove(0)),
		_ => bail!(
			"multiple devices connected — pass --serial (found: {})",
			devices.join(", ")
		),
	}
}

/// Device ids in state `device` from `adb devices` stdout.
pub fn parse_devices(stdout: &str) -> Vec<String> {
	let mut devices = Vec::new();
	for line in stdout.lines().skip(1) {
		let mut it = line.split_whitespace();
		if let (Some(id), Some(state)) = (it.next(), it.next()) {
			if state == "device" {
				devices.push(id.to_string());
			}
		}
	}
	devices
}

/// A handle on one device: every command goes through `-s <serial>`.
pub struct Adb {
	serial: String,
}

impl Adb {
	pub fn new(serial: impl Into<String>) -> Self {
		Self { serial: serial.into() }
	}

	pub fn serial(&self) -> &str {
		&self.serial
	}

	pub fn command(&self) -> Command {
		let mut c = Command::new("adb");
		c.args(["-s", &self.serial]);
		c
	}

	/// `adb shell <args>`, stdout as String. Fails on non-zero exit.
	pub fn shell(&self, args: &[&str]) -> Result<String> {
		let out = self
			.command()
			.arg("shell")
			.args(args)
			.output()
			.with_context(|| format!("adb shell {}", args.join(" ")))?;
		if !out.status.success() {
			bail!(
				"adb shell {} failed: {}",
				args.join(" "),
				String::from_utf8_lossy(&out.stderr).trim()
			);
		}
		Ok(String::from_utf8_lossy(&out.stdout).into_owned())
	}

	/// `adb shell` with a hard timeout (uiautomator dump likes to hang).
	pub fn shell_timeout(&self, args: &[&str], timeout: Duration) -> Result<String> {
		let mut child = self
			.command()
			.arg("shell")
			.args(args)
			.stdin(Stdio::null())
			.stdout(Stdio::piped())
			.stderr(Stdio::piped())
			.spawn()
			.with_context(|| format!("adb shell {}", args.join(" ")))?;
		let start = Instant::now();
		loop {
			if let Some(status) = child.try_wait()? {
				let mut out = String::new();
				if let Some(mut so) = child.stdout.take() {
					let _ = so.read_to_string(&mut out);
				}
				if !status.success() {
					bail!("adb shell {} failed: {out}", args.join(" "));
				}
				return Ok(out);
			}
			if start.elapsed() > timeout {
				let _ = child.kill();
				let _ = child.wait();
				bail!("adb shell {} timed out after {timeout:?}", args.join(" "));
			}
			std::thread::sleep(Duration::from_millis(50));
		}
	}

	/// `adb shell` ignoring failures.
	pub fn shell_quiet(&self, args: &[&str]) {
		let _ = self.shell(args);
	}

	/// `adb shell <args>` ignoring non-zero exit (`(adb shell ... || true)`).
	/// For commands whose failure is a normal outcome (`pm path`, `pidof`).
	pub fn shell_opt(&self, args: &[&str]) -> Option<String> {
		self.command()
			.arg("shell")
			.args(args)
			.output()
			.ok()
			.map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
	}

	/// `adb exec-out <args>`, raw stdout bytes (screencap, cat).
	pub fn exec_out(&self, args: &[&str]) -> Result<Vec<u8>> {
		let out = self
			.command()
			.arg("exec-out")
			.args(args)
			.output()
			.with_context(|| format!("adb exec-out {}", args.join(" ")))?;
		if !out.status.success() {
			bail!(
				"adb exec-out {} failed: {}",
				args.join(" "),
				String::from_utf8_lossy(&out.stderr).trim()
			);
		}
		Ok(out.stdout)
	}
}

/// Shared handle on a spawned child so the main thread can kill it.
pub struct ChildHandle {
	child: std::sync::Mutex<Child>,
}

impl ChildHandle {
	pub fn new(child: Child) -> Self {
		Self { child: std::sync::Mutex::new(child) }
	}

	pub fn kill(&self) {
		if let Ok(mut c) = self.child.lock() {
			let _ = c.kill();
			let _ = c.wait();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_devices_keeps_only_ready() {
		let out = "List of devices attached\n\
		           emulator-5554\tdevice\n\
		           192.168.1.4:5555\tdevice\n\
		           0a1b2c3d\tunauthorized\n\
		           \n";
		assert_eq!(
			parse_devices(out),
			vec!["emulator-5554".to_string(), "192.168.1.4:5555".to_string()]
		);
	}

	#[test]
	fn parse_devices_empty() {
		assert!(parse_devices("List of devices attached\n").is_empty());
	}

	#[test]
	fn pick_serial_fails_gracefully_without_adb() {
		// Can't assert success (depends on env); just ensure it doesn't panic on parse.
		if Command::new("adb").arg("--version").output().is_err() {
			assert!(pick_serial(None).is_err());
		}
	}
}
