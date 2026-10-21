//! Device-side actions built on `adbkit`'s generic plumbing.
//!
//! `adbkit` owns serial picking, shell/exec-out wrappers and launcher
//! resolution; the functions here are the uitool-specific verbs
//! (wake, press, screencap).

use anyhow::Result;

use adbkit::Adb;

/// Press at (x, y): quick `input tap` when `duration` is 0, hold `duration` ms
/// otherwise (`input swipe` with identical endpoints is the canonical hold).
pub fn press(dev: &Adb, x: i32, y: i32, duration: u64) -> Result<()> {
	if duration == 0 {
		dev.shell(&["input", "tap", &x.to_string(), &y.to_string()])?;
	} else {
		dev.shell(&[
			"input",
			"swipe",
			&x.to_string(),
			&y.to_string(),
			&x.to_string(),
			&y.to_string(),
			&duration.to_string(),
		])?;
	}
	Ok(())
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
