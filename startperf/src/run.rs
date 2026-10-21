//! The measurement loop: force-stop (cold), `am start -W`, parse, collect.
//!
//! The device interaction goes through the [`Device`] trait so the loop's
//! decisions — cold vs warm, what to do with a "brought to the front" run —
//! are unit-tested without a device.

use anyhow::{Context, Result};
use serde::Serialize;

use adbkit::Adb;

use crate::parse::parse_start_output;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
	/// `am force-stop` before every run: process absent, full init path.
	Cold,
	/// Relaunch as-is: process already resident.
	Warm,
}

impl Mode {
	pub fn as_str(&self) -> &'static str {
		match self {
			Mode::Cold => "cold",
			Mode::Warm => "warm",
		}
	}
}

/// One measured launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sample {
	pub run: u32,
	pub total_ms: u64,
	pub wait_ms: Option<u64>,
	pub this_ms: Option<u64>,
	pub launch_state: Option<String>,
	/// True when Android admitted it did not really start the activity.
	pub brought_to_front: bool,
}

/// What a benchmark produced, plus everything it wants to complain about.
#[derive(Debug, Default)]
pub struct Outcome {
	pub samples: Vec<Sample>,
	/// Runs dropped from the stats, in order, with the reason.
	pub skipped: Vec<(u32, String)>,
}

/// The device operations the loop needs. One method per intent, so a fake
/// device is a 20-line struct.
pub trait Device {
	fn force_stop(&mut self, package: &str) -> Result<()>;
	/// Raw stdout of `am start -W -n <component>`.
	fn start_w(&mut self, component: &str) -> Result<String>;
	/// Send the user to HOME, leaving the app in the background *alive*.
	///
	/// This is what makes a warm run a warm run: the process stays resident
	/// and the activity is recreated on the next start. Relaunching while the
	/// app is already top-most launches nothing at all and Android reports
	/// `TotalTime: 0`.
	fn home(&mut self);
	fn settle(&mut self, ms: u64);
}

/// Real device: `adb shell` through `adbkit`.
pub struct AdbDevice<'a> {
	pub dev: &'a Adb,
}

impl Device for AdbDevice<'_> {
	fn force_stop(&mut self, package: &str) -> Result<()> {
		self.dev.shell(&["am", "force-stop", package]).map(|_| ())
	}

	fn start_w(&mut self, component: &str) -> Result<String> {
		self.dev
			.shell_timeout(
				&["am", "start", "-W", "-n", component],
				std::time::Duration::from_secs(30),
			)
			.with_context(|| format!("am start -W -n {component}"))
	}

	fn settle(&mut self, ms: u64) {
		if ms > 0 {
			std::thread::sleep(std::time::Duration::from_millis(ms));
		}
	}

	fn home(&mut self) {
		self.dev.shell_quiet(&["input", "keyevent", "KEYCODE_HOME"]);
	}
}

/// Run `runs` measured launches and collect the usable ones.
///
/// A run is skipped (kept in [`Outcome::skipped`], excluded from stats) when
/// Android did not actually launch anything:
///
/// - cold mode + *brought to the front* — the `force-stop` had no effect, so
///   it would measure a resume, not a cold start;
/// - any mode + *top-most instance* — the activity was already on top and
///   Android merely delivered the intent, reporting a meaningless `TotalTime`
///   of 0.
pub fn benchmark<D: Device>(
	dev: &mut D,
	package: &str,
	component: &str,
	runs: u32,
	mode: Mode,
	settle_ms: u64,
) -> Result<Outcome> {
	let mut outcome = Outcome::default();

	for run in 1..=runs {
		match mode {
			Mode::Cold => {
				dev.force_stop(package)?;
				dev.settle(settle_ms);
			}
			// HOME backgrounds the activity without killing the process, so the
			// next start recreates it against a warm process — a real warm
			// start. Without this the app stays top-most and Android reports
			// `TotalTime: 0` ("top-most instance", launched nothing).
			Mode::Warm => {
				dev.home();
				dev.settle(settle_ms);
			}
		}

		let raw = dev.start_w(component)?;
		let report = parse_start_output(&raw)?;

		if report.already_on_top() {
			outcome.skipped.push((run, "already top-most — Android launched nothing".into()));
			dev.settle(settle_ms);
			continue;
		}
		if report.brought_to_front() && mode == Mode::Cold {
			outcome.skipped.push((run, "brought to the front — force-stop had no effect".into()));
			dev.settle(settle_ms);
			continue;
		}

		// Read the flag before the fields are moved into the sample.
		let brought_to_front = report.brought_to_front();
		outcome.samples.push(Sample {
			run,
			total_ms: report.total_ms()?,
			wait_ms: report.wait_time,
			this_ms: report.this_time,
			launch_state: report.launch_state,
			brought_to_front,
		});
		dev.settle(settle_ms);
	}

	Ok(outcome)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::cell::RefCell;

	/// Scriptable device: canned `am start -W` outputs, recorded calls.
	#[derive(Default)]
	struct FakeDevice {
		starts: RefCell<Vec<String>>,
		force_stops: RefCell<Vec<String>>,
		homes: RefCell<u32>,
		settles: RefCell<Vec<u64>>,
		script: Vec<String>,
	}

	impl Device for FakeDevice {
		fn force_stop(&mut self, package: &str) -> Result<()> {
			self.force_stops.borrow_mut().push(package.to_string());
			Ok(())
		}
		fn start_w(&mut self, _component: &str) -> Result<String> {
			self.starts.borrow_mut().push(String::new());
			Ok(self.script.remove(0))
		}
		fn settle(&mut self, ms: u64) {
			self.settles.borrow_mut().push(ms);
		}
		fn home(&mut self) {
			*self.homes.borrow_mut() += 1;
		}
	}

	fn cold(total: u32) -> String {
		format!("Status: ok\nLaunchState: COLD\nTotalTime: {total}\nWaitTime: {}\n", total + 10)
	}

	/// The HOT resume a real device answers after HOME (captured on Android 15).
	fn hot(total: u32) -> String {
		format!(
			"Warning: Activity not started, its current task has been brought to the front\n\
			 Status: ok\nLaunchState: HOT\nTotalTime: {total}\nWaitTime: {}\n",
			total + 10
		)
	}

	/// The "launched nothing" answer when the app is already top-most
	/// (Android 15 wording, captured on a real device).
	fn on_top() -> String {
		"Warning: Activity not started, intent has been delivered to currently running top-most instance.\n\
		 Status: ok\nLaunchState: UNKNOWN (0)\nTotalTime: 0\nWaitTime: 12\n".into()
	}

	#[test]
	fn cold_mode_force_stops_before_every_run() {
		let mut fake = FakeDevice { script: vec![cold(100), cold(200)], ..Default::default() };
		let out = benchmark(&mut fake, "com.foo", "com.foo/.Main", 2, Mode::Cold, 500).unwrap();
		assert_eq!(force_stops(&fake), vec!["com.foo", "com.foo"]);
		assert_eq!(totals(&out), vec![100, 200]);
		// Settled after the force-stop and after each measurement.
		assert_eq!(*fake.settles.borrow(), vec![500, 500, 500, 500]);
		// Cold mode must not press HOME.
		assert_eq!(*fake.homes.borrow(), 0);
	}

	#[test]
	fn warm_mode_goes_home_and_never_force_stops() {
		let mut fake = FakeDevice { script: vec![hot(110)], ..Default::default() };
		let out = benchmark(&mut fake, "com.foo", "com.foo/.Main", 1, Mode::Warm, 0).unwrap();
		assert!(fake.force_stops.borrow().is_empty());
		// HOME is what leaves the app in the background *alive* — without it
		// the next start reports `TotalTime: 0` and launches nothing.
		assert_eq!(*fake.homes.borrow(), 1);
		assert_eq!(totals(&out), vec![110]);
		// Zero settle is still forwarded, but means "don't sleep": the real
		// device turns 0 into no-op, and the fake records the value.
		assert_eq!(*fake.settles.borrow(), vec![0, 0]);
	}

	#[test]
	fn warm_mode_presses_home_once_per_run() {
		let mut fake =
			FakeDevice { script: vec![hot(100), hot(110), hot(120)], ..Default::default() };
		benchmark(&mut fake, "com.foo", "com.foo/.Main", 3, Mode::Warm, 0).unwrap();
		assert_eq!(*fake.homes.borrow(), 3);
	}

	#[test]
	fn cold_run_brought_to_front_is_skipped_not_counted() {
		let mut fake = FakeDevice { script: vec![hot(3), cold(900)], ..Default::default() };
		let out = benchmark(&mut fake, "com.foo", "com.foo/.Main", 2, Mode::Cold, 0).unwrap();
		// A bogus 3 ms "cold start" must not pollute the stats.
		assert_eq!(totals(&out), vec![900]);
		assert_eq!(out.skipped.len(), 1);
		assert_eq!(out.skipped[0].0, 1);
		assert!(out.skipped[0].1.contains("brought to the front"));
	}

	#[test]
	fn already_top_most_is_skipped_in_any_mode() {
		// Android launched nothing: counting it would put a fake 0 ms in the
		// stats (this is exactly what real-device testing caught).
		// The follow-up run must be plausible for the mode under test, since
		// cold legitimately drops "brought to the front" answers too.
		for (mode, second) in [(Mode::Cold, cold(900)), (Mode::Warm, hot(110))] {
			let mut fake = FakeDevice { script: vec![on_top(), second], ..Default::default() };
			let out = benchmark(&mut fake, "com.foo", "com.foo/.Main", 2, mode, 0).unwrap();
			assert_eq!(out.skipped.len(), 1, "{mode:?}");
			assert!(out.skipped[0].1.contains("top-most"), "{:?}", out.skipped[0].1);
			// The valid run is still measured.
			assert_eq!(totals(&out).len(), 1, "{mode:?}");
			assert!(totals(&out)[0] > 0, "{mode:?}");
		}
	}

	#[test]
	fn warm_resumed_run_is_still_a_sample() {
		// After HOME, Android answers with the "brought to the front" warning
		// *and* LaunchState HOT with a real TotalTime — that is a warm start.
		let mut fake = FakeDevice { script: vec![hot(110)], ..Default::default() };
		let out = benchmark(&mut fake, "com.foo", "com.foo/.Main", 1, Mode::Warm, 0).unwrap();
		assert_eq!(totals(&out), vec![110]);
		assert!(out.skipped.is_empty());
		assert!(out.samples[0].brought_to_front);
		assert_eq!(out.samples[0].launch_state.as_deref(), Some("HOT"));
	}

	#[test]
	fn sample_carries_all_reported_fields() {
		let mut fake = FakeDevice { script: vec![cold(842)], ..Default::default() };
		let out = benchmark(&mut fake, "p", "p/.A", 1, Mode::Cold, 0).unwrap();
		let s = &out.samples[0];
		assert_eq!(s.run, 1);
		assert_eq!(s.total_ms, 842);
		assert_eq!(s.wait_ms, Some(852));
		assert_eq!(s.this_ms, None);
		assert_eq!(s.launch_state.as_deref(), Some("COLD"));
	}

	#[test]
	fn device_error_aborts_the_benchmark() {
		// Unparseable output (e.g. adb noise) must abort, not become a 0 ms run.
		let mut fake =
			FakeDevice { script: vec!["adb: device offline".into()], ..Default::default() };
		let err = benchmark(&mut fake, "p", "p/.A", 1, Mode::Cold, 0).unwrap_err();
		assert!(err.to_string().contains("unrecognized"), "{err}");
	}

	fn totals(out: &Outcome) -> Vec<u64> {
		out.samples.iter().map(|s| s.total_ms).collect()
	}
	fn force_stops(fake: &FakeDevice) -> Vec<String> {
		fake.force_stops.borrow().clone()
	}
}
