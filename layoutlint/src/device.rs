//! Device access: the dump itself, and the density to convert px to dp.

use std::time::Duration;

use anyhow::{bail, Context, Result};

use adbkit::Adb;

/// Dump the current UI via /sdcard — the route that works everywhere;
/// `uiautomator dump` to stdout truncates on some devices.
pub fn dump(dev: &Adb) -> Result<String> {
	dev.shell_timeout(&["uiautomator", "dump", "/sdcard/layoutlint.xml"], Duration::from_secs(8))
		.context("uiautomator dump")?;
	let xml = dev.exec_out(&["cat", "/sdcard/layoutlint.xml"])?;
	let xml = String::from_utf8_lossy(&xml).into_owned();
	if !xml.contains("<hierarchy") {
		let preview: String = xml.chars().take(200).collect();
		bail!("dump is not a hierarchy (screen off or locked?): {preview}");
	}
	Ok(xml)
}

/// Density the running UI is drawn at, in dpi.
pub fn device_density(dev: &Adb) -> Result<u32> {
	let out = dev.shell(&["wm", "density"])?;
	parse_wm_density(&out).with_context(|| format!("unrecognized `wm density` output: {out:?}"))
}

/// dpi from `wm density`: the override when there is one — that is the
/// density apps actually draw at — else the physical density.
pub fn parse_wm_density(out: &str) -> Option<u32> {
	let mut physical = None;
	let mut over = None;
	for line in out.lines() {
		if let Some(v) = line.strip_prefix("Physical density:") {
			physical = v.trim().parse().ok();
		} else if let Some(v) = line.strip_prefix("Override density:") {
			over = v.trim().parse().ok();
		}
	}
	over.or(physical)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn override_density_wins_over_physical() {
		let out = "Physical density: 440\nOverride density: 335\n";
		assert_eq!(parse_wm_density(out), Some(335));
	}

	#[test]
	fn physical_density_alone_is_used() {
		assert_eq!(parse_wm_density("Physical density: 440"), Some(440));
	}

	#[test]
	fn garbage_is_none() {
		assert_eq!(parse_wm_density("no device"), None);
	}
}
