//! Parsing of `am start -W` output.
//!
//! `-W` makes `am start` block until the launch is finished and report the
//! timings. Two shapes exist in the wild:
//!
//! ```text
//! Starting: Intent { cmp=com.foo/.MainActivity }
//! Status: ok
//! LaunchState: COLD
//! Activity: com.foo/.MainActivity
//! TotalTime: 842
//! WaitTime: 856
//! Complete
//! ```
//!
//! and, on older releases, just the timing block (`ThisTime:` / `TotalTime:` /
//! `WaitTime:`) with no `Status:`. Both are accepted: field names are matched
//! case-insensitively, with optional whitespace around the separator.
//!
//! Android may also prepend a `Warning:` line when it did *not* do a fresh
//! start. Its wording changed across releases (see [`StartReport::brought_to_front`]
//! and [`StartReport::already_on_top`]), so the raw line is kept and matched
//! by substring rather than by exact sentence.

use anyhow::{bail, Context, Result};

/// One `am start -W` measurement. Every field is optional because the set of
/// keys Android reports has grown over releases.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StartReport {
	/// Time to draw the first frame of the launched activity (ms).
	pub total_time: Option<u64>,
	/// Time until the launch was fully complete, including later windows (ms).
	pub wait_time: Option<u64>,
	/// Time to launch only the top activity (ms).
	pub this_time: Option<u64>,
	pub status: Option<String>,
	/// Device-reported launch state (COLD / WARM / HOT) — a cross-check on the
	/// mode we asked for.
	pub launch_state: Option<String>,
	pub activity: Option<String>,
	/// Text of the `Warning:` line, when Android declined to do a fresh start.
	/// The wording varies by release (see the two helpers), so the raw line is
	/// kept and matched by substring.
	pub warning: Option<String>,
}

impl StartReport {
	/// The number a benchmark is about: `TotalTime`.
	pub fn total_ms(&self) -> Result<u64> {
		self.total_time.context("no TotalTime in `am start -W` output")
	}

	/// The activity was already the running top-most instance: Android merely
	/// delivered the intent, launched nothing, and reports `TotalTime: 0`.
	///
	/// Android 15 wording: *"intent has been delivered to currently running
	/// top-most instance"*.
	pub fn already_on_top(&self) -> bool {
		self.warning.as_deref().is_some_and(|w| w.contains("top-most instance"))
	}

	/// The task was brought forward instead of being cold-started.
	///
	/// Android 14 and earlier: *"its current task has been brought to the
	/// front"*. Also emitted on a legitimate warm/HOT resume, so callers must
	/// weigh it against the mode they asked for.
	pub fn brought_to_front(&self) -> bool {
		self.warning.as_deref().is_some_and(|w| w.contains("brought to the front"))
	}
}

/// Parse one `am start -W` output. Errors when nothing recognizable was
/// reported, or when `Status:` is anything but `ok`.
pub fn parse_start_output(out: &str) -> Result<StartReport> {
	let mut report = StartReport::default();

	for line in out.lines() {
		let line = line.trim();
		if let Some(rest) = line.strip_prefix("Warning:") {
			// Keep the last warning: the two shapes below both start with
			// "Activity not started", and the rest of the sentence is what
			// distinguishes "launched nothing" from "resumed the task".
			report.warning = Some(rest.trim().to_string());
			continue;
		}
		let Some((key, value)) = split_kv(line) else { continue };
		match key.to_ascii_lowercase().as_str() {
			"status" => report.status = Some(value.to_string()),
			"launchstate" => report.launch_state = Some(value.to_string()),
			"activity" => report.activity = Some(value.to_string()),
			"totaltime" => report.total_time = parse_ms(value),
			"waittime" => report.wait_time = parse_ms(value),
			"thistime" => report.this_time = parse_ms(value),
			_ => {}
		}
	}

	// Nothing we understand: the device spoke a format we don't know, and
	// silently reporting a "0 ms start" would poison the benchmark.
	if report.total_time.is_none() && report.status.is_none() && report.activity.is_none() {
		bail!("unrecognized `am start -W` output:\n{out}");
	}
	if let Some(status) = &report.status {
		if !status.eq_ignore_ascii_case("ok") {
			bail!("`am start` reported status {status:?}:\n{out}");
		}
	}
	Ok(report)
}

/// `Key: value`, `Key:value` or `Key=value` — first separator wins.
fn split_kv(line: &str) -> Option<(&str, &str)> {
	let (key, rest) = line.split_once([':', '='])?;
	let key = key.trim();
	// Real keys are single words; this drops prose lines such as
	// `Starting: Intent { cmp=... }` (handled: key "Starting" is unknown)
	// and any line whose left side contains a sentence.
	if key.is_empty() || key.contains(' ') {
		return None;
	}
	Some((key, rest.trim()))
}

/// Leading digits of a value: `842`, `842 ms` → milliseconds. Garbage → None.
fn parse_ms(value: &str) -> Option<u64> {
	let digits: String = value.chars().take_while(|c| c.is_ascii_digit()).collect();
	digits.parse().ok()
}

#[cfg(test)]
mod tests {
	use super::*;

	const MODERN: &str = "Starting: Intent { cmp=com.foo/.MainActivity }\n\
	                      Status: ok\n\
	                      LaunchState: COLD\n\
	                      Activity: com.foo/.MainActivity\n\
	                      TotalTime: 842\n\
	                      WaitTime: 856\n\
	                      Complete\n";

	#[test]
	fn parses_modern_output() {
		let r = parse_start_output(MODERN).unwrap();
		assert_eq!(r.total_time, Some(842));
		assert_eq!(r.wait_time, Some(856));
		// ThisTime is not reported by every release.
		assert_eq!(r.this_time, None);
		assert_eq!(r.status.as_deref(), Some("ok"));
		assert_eq!(r.launch_state.as_deref(), Some("COLD"));
		assert_eq!(r.activity.as_deref(), Some("com.foo/.MainActivity"));
		assert!(!r.brought_to_front());
		assert_eq!(r.total_ms().unwrap(), 842);
	}

	#[test]
	fn parses_legacy_timing_block() {
		let out = "thisTime: 810\nTotalTime: 842\nWaitTime: 845\nComplete\n";
		let r = parse_start_output(out).unwrap();
		assert_eq!(r.this_time, Some(810));
		assert_eq!(r.total_time, Some(842));
		assert_eq!(r.wait_time, Some(845));
		// Legacy shape has no Status: at all — that is not an error.
		assert_eq!(r.status, None);
	}

	#[test]
	fn tolerates_tight_and_mixed_case_separators() {
		let r = parse_start_output("totalTime:842\nSTATUS: ok\nWaitTime = 845").unwrap();
		assert_eq!(r.total_time, Some(842));
		assert_eq!(r.wait_time, Some(845));
		assert_eq!(r.status.as_deref(), Some("ok"));
	}

	#[test]
	fn ms_suffix_is_tolerated() {
		let r = parse_start_output("TotalTime: 842 ms\nStatus: ok").unwrap();
		assert_eq!(r.total_time, Some(842));
	}

	#[test]
	fn detects_brought_to_front_warning() {
		let out = "Warning: Activity not started, its current task has been brought to the front\n\
		           Status: ok\n\
		           TotalTime: 3\n";
		let r = parse_start_output(out).unwrap();
		assert!(r.brought_to_front());
		assert!(!r.already_on_top());
		assert_eq!(r.total_time, Some(3));
	}

	#[test]
	fn detects_topmost_instance_warning() {
		// Android 15 wording, captured on a real device (Redmi 23124RA7EO).
		let out = "Starting: Intent { cmp=com.termux/.app.TermuxActivity }\n\
		           Warning: Activity not started, intent has been delivered to currently running top-most instance.\n\
		           Status: ok\n\
		           LaunchState: UNKNOWN (0)\n\
		           Activity: com.termux/.app.TermuxActivity\n\
		           TotalTime: 0\n\
		           WaitTime: 12\n\
		           Complete\n";
		let r = parse_start_output(out).unwrap();
		// Must be recognized: it was silently counted as a 0 ms launch before.
		assert!(r.already_on_top());
		assert!(!r.brought_to_front());
		assert_eq!(r.total_time, Some(0));
		assert_eq!(r.launch_state.as_deref(), Some("UNKNOWN (0)"));
	}

	#[test]
	fn unrecognized_warning_is_kept_but_flags_stay_false() {
		let r = parse_start_output("Warning: something else\nStatus: ok\nTotalTime: 5").unwrap();
		assert_eq!(r.warning.as_deref(), Some("something else"));
		assert!(!r.already_on_top());
		assert!(!r.brought_to_front());
	}

	#[test]
	fn no_warning_leaves_flags_false() {
		let r = parse_start_output(MODERN).unwrap();
		assert_eq!(r.warning, None);
		assert!(!r.already_on_top());
		assert!(!r.brought_to_front());
	}

	#[test]
	fn starting_line_with_equals_does_not_clobber_fields() {
		// `Starting: Intent { cmp=com.foo/.MainActivity }` has an '=' inside
		// its value; it must not be parsed as a key=value pair.
		let out = "Starting: Intent { cmp=com.foo/.MainActivity }\n\
		           Warning: Activity not started, its current task has been brought to the front\n\
		           Status: ok\n\
		           TotalTime: 3\n";
		let r = parse_start_output(out).unwrap();
		assert_eq!(r.activity, None);
		assert_eq!(r.total_time, Some(3));
		assert!(r.brought_to_front());
	}

	#[test]
	fn non_ok_status_is_an_error() {
		let out =
			"Status: error\nError type 3\nError: Activity class {com.foo/.Main} does not exist.\n";
		let err = parse_start_output(out).unwrap_err();
		assert!(err.to_string().contains("\"error\""), "{err}");
	}

	#[test]
	fn unrecognized_output_is_an_error() {
		// `adb: device offline` style noise must not become a 0 ms measurement.
		let err = parse_start_output("adb: device offline\n").unwrap_err();
		assert!(err.to_string().contains("unrecognized"), "{err}");
	}

	#[test]
	fn total_ms_errors_without_total_time() {
		let r = StartReport { status: Some("ok".into()), ..Default::default() };
		assert!(r.total_ms().is_err());
	}

	#[test]
	fn empty_output_is_an_error() {
		assert!(parse_start_output("").is_err());
	}
}
