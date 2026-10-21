//! Pure command construction — what runs on the device, and how one
//! reports it. No `adb` in here, so every rule is unit-tested.
//!
//! The host hands the argv to `adb shell` as separate entries, which adb
//! joins with spaces for the *device* shell to re-parse: nothing is
//! escaped on the way. Input is therefore validated, never trusted — a
//! package name cannot carry shell metacharacters, and a URI travels
//! single-quoted.

use anyhow::{bail, Result};

/// The lifecycle actions. `Open` is the one carrying a URI instead of a
/// package name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
	ClearData,
	ClearCache,
	Kill,
	Restart,
	Uninstall,
	Open,
}

/// `adb shell …` argv for `action` — the package name, or the URI for
/// `Open`.
pub fn argv(action: Action, arg: &str) -> Result<Vec<String>> {
	fn v(parts: &[&str]) -> Vec<String> {
		parts.iter().map(|s| s.to_string()).collect()
	}
	Ok(match action {
		Action::ClearData => {
			check_package(arg)?;
			v(&["pm", "clear", arg])
		}
		Action::ClearCache => {
			check_package(arg)?;
			v(&["pm", "clear", "--cache-only", arg])
		}
		// force-stop is idempotent: a not-running app is a success, so
		// kill and the stop half of restart share the command.
		Action::Kill | Action::Restart => {
			check_package(arg)?;
			v(&["am", "force-stop", arg])
		}
		Action::Uninstall => {
			check_package(arg)?;
			v(&["pm", "uninstall", arg])
		}
		Action::Open => {
			let uri = quoted_uri(arg)?;
			v(&["am", "start", "-a", "android.intent.action.VIEW", "-d", &uri])
		}
	})
}

/// `am start -n <component>` for a launcher activity resolved at run time.
pub fn start_argv(component: &str) -> Vec<String> {
	["am", "start", "-n", component].iter().map(|s| s.to_string()).collect()
}

/// Package names `pm`/`am` can carry: the Android package charset, and
/// nothing a shell would read as syntax — no leading `-` either, so the
/// name can never look like a flag to `pm`.
pub fn check_package(pkg: &str) -> Result<()> {
	let ok = !pkg.is_empty()
		&& !pkg.starts_with('-')
		&& pkg.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
	if !ok {
		bail!("not a package name: {pkg:?}");
	}
	Ok(())
}

/// A deep link as the device shell must see it: single-quoted, so `&`
/// and friends stay inside the URI. A quote or whitespace inside would
/// break out of the quoting — those are rejected instead.
pub fn quoted_uri(uri: &str) -> Result<String> {
	if uri.is_empty() || uri.contains('\'') || uri.chars().any(char::is_whitespace) {
		bail!("URI must be non-empty, without quotes or spaces: {uri:?}");
	}
	Ok(format!("'{uri}'"))
}

/// `[serial] detail` when one call spans several devices, bare detail on
/// a single one.
pub fn line(serial: &str, multi: bool, detail: &str) -> String {
	if multi {
		format!("[{serial}] {detail}")
	} else {
		detail.to_string()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn argv_maps_every_action() {
		assert_eq!(argv(Action::ClearData, "com.x").unwrap(), ["pm", "clear", "com.x"]);
		assert_eq!(
			argv(Action::ClearCache, "com.x").unwrap(),
			["pm", "clear", "--cache-only", "com.x"]
		);
		assert_eq!(argv(Action::Kill, "com.x").unwrap(), ["am", "force-stop", "com.x"]);
		assert_eq!(argv(Action::Uninstall, "com.x").unwrap(), ["pm", "uninstall", "com.x"]);
		assert_eq!(
			argv(Action::Open, "myapp://detail/7").unwrap(),
			["am", "start", "-a", "android.intent.action.VIEW", "-d", "'myapp://detail/7'"]
		);
	}

	#[test]
	fn restart_stops_the_app_and_starts_from_the_component() {
		assert_eq!(argv(Action::Restart, "com.x").unwrap(), ["am", "force-stop", "com.x"]);
		assert_eq!(start_argv("com.x/.MainActivity"), ["am", "start", "-n", "com.x/.MainActivity"]);
	}

	#[test]
	fn a_package_name_cannot_carry_shell_syntax() {
		for bad in ["", "-f", "com.x; reboot", "com x", "com.x|id", "com.x'", "$(id)"] {
			assert!(argv(Action::Kill, bad).is_err(), "{bad:?}");
			assert!(argv(Action::Uninstall, bad).is_err(), "{bad:?}");
		}
	}

	#[test]
	fn a_uri_is_quoted_and_rejects_breakouts() {
		assert_eq!(quoted_uri("https://x.test?a=1&b=2").unwrap(), "'https://x.test?a=1&b=2'");
		for bad in ["", "myapp://it's", "myapp://two words"] {
			assert!(quoted_uri(bad).is_err(), "{bad:?}");
		}
	}

	#[test]
	fn line_names_the_serial_only_when_several_devices() {
		assert_eq!(line("SER123", false, "stopped"), "stopped");
		assert_eq!(line("SER123", true, "stopped"), "[SER123] stopped");
	}
}
