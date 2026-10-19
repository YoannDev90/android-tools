//! Device-side actions built on `adbkit`'s generic plumbing.
//!
//! `adbkit` owns serial picking and shell/exec-out wrappers; the functions
//! here are the uitool-specific verbs (wake, press, launcher resolution).

use anyhow::{Context, Result};

use adbkit::Adb;

/// Press at (x, y): quick `input tap` when `duration` is 0, hold `duration` ms
/// otherwise (`input swipe` with identical endpoints is the canonical hold).
pub fn press(dev: &Adb, x: i32, y: i32, duration: u64) -> Result<()> {
	if duration == 0 {
		dev.shell(&["input", "tap", &x.to_string(), &y.to_string()])?;
	} else {
		dev.shell(&[
			"input", "swipe", &x.to_string(), &y.to_string(), &x.to_string(), &y.to_string(),
			&duration.to_string(),
		])?;
	}
	Ok(())
}

/// Launcher activity of a package (`cmd package resolve-activity --brief`).
pub fn resolve_launcher(dev: &Adb, package: &str) -> Result<String> {
	let out = dev
		.shell(&["cmd", "package", "resolve-activity", "--brief", package])
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
pub fn wake(dev: &Adb) -> Result<()> {
	dev.shell_quiet(&["input", "keyevent", "KEYCODE_WAKEUP"]);
	dev.shell_quiet(&["wm", "dismiss-keyguard"]);
	dev.shell_quiet(&["cmd", "statusbar", "collapse"]);
	Ok(())
}

pub fn screencap(dev: &Adb) -> Result<Vec<u8>> {
	dev.exec_out(&["screencap", "-p"])
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
