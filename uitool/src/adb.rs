use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

pub fn pick_serial(explicit: Option<&str>) -> Result<String> {
	if let Some(s) = explicit {
		return Ok(s.to_string());
	}
	let out = Command::new("adb")
		.args(["devices"])
		.output()
		.context("adb not found in PATH — install platform-tools")?;
	let mut devices = Vec::new();
	for line in String::from_utf8_lossy(&out.stdout).lines().skip(1) {
		let mut it = line.split_whitespace();
		if let (Some(id), Some(state)) = (it.next(), it.next()) {
			if state == "device" {
				devices.push(id.to_string());
			}
		}
	}
	match devices.len() {
		0 => bail!("no adb device connected (or unauthorized)"),
		1 => Ok(devices.remove(0)),
		_ => bail!("multiple devices — pass --serial (found: {})", devices.join(", ")),
	}
}

fn base(serial: &str) -> Command {
	let mut c = Command::new("adb");
	c.args(["-s", serial]);
	c
}

pub fn shell(serial: &str, args: &[&str]) -> Result<String> {
	let out = base(serial)
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
pub fn shell_timeout(serial: &str, args: &[&str], timeout: Duration) -> Result<String> {
	let mut child = base(serial)
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

pub fn shell_quiet(serial: &str, args: &[&str]) {
	let _ = shell(serial, args);
}

/// Binary stdout (screencap, exec-out cat).
pub fn exec_out(serial: &str, args: &[&str]) -> Result<Vec<u8>> {
	let out = base(serial)
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

/// Press at (x, y): quick `input tap` when `duration` is 0, hold `duration` ms
/// otherwise (`input swipe` with identical endpoints is the canonical hold).
pub fn press(serial: &str, x: i32, y: i32, duration: u64) -> Result<()> {
	if duration == 0 {
		shell(serial, &["input", "tap", &x.to_string(), &y.to_string()])?;
	} else {
		shell(
			serial,
			&[
				"input", "swipe", &x.to_string(), &y.to_string(), &x.to_string(), &y.to_string(),
				&duration.to_string(),
			],
		)?;
	}
	Ok(())
}

/// Launcher activity of a package (`cmd package resolve-activity --brief`).
pub fn resolve_launcher(serial: &str, package: &str) -> Result<String> {
	let out = shell(serial, &["cmd", "package", "resolve-activity", "--brief", package])
		.with_context(|| format!("resolve-activity {package}"))?;
	parse_component(&out)
		.map(str::to_string)
		.with_context(|| format!("no launcher activity for {package:?} — pass the full component (pkg/.Activity)"))
}

/// Last `package/class` line of a `resolve-activity --brief` dump.
fn parse_component(out: &str) -> Option<&str> {
	out.lines().map(str::trim).rfind(|l| l.contains('/'))
}

/// Wake screen, dismiss keyguard, collapse status bar / notifications.
pub fn wake(serial: &str) -> Result<()> {
	shell_quiet(serial, &["input", "keyevent", "KEYCODE_WAKEUP"]);
	shell_quiet(serial, &["wm", "dismiss-keyguard"]);
	shell_quiet(serial, &["cmd", "statusbar", "collapse"]);
	Ok(())
}

pub fn screencap(serial: &str) -> Result<Vec<u8>> {
	exec_out(serial, &["screencap", "-p"])
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_component_takes_last_slash_line() {
		let out = "priority=0 preferredOrder=0 match=0x108000 specificIndex=-1 isDefault=true\ncom.foo/.MainActivity\n";
		assert_eq!(parse_component(out), Some("com.foo/.MainActivity"));
	}

	#[test]
	fn parse_component_skips_lines_without_component() {
		let out = "No activity found\ncom.foo/com.foo.Other\n";
		assert_eq!(parse_component(out), Some("com.foo/com.foo.Other"));
	}

	#[test]
	fn parse_component_none_without_activity() {
		assert_eq!(parse_component("No activity found\n"), None);
		assert_eq!(parse_component(""), None);
	}
}
