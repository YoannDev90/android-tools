//! One `Sample` per app per tick, rendered as CSV rows or a JSON array,
//! plus the end-of-run per-app `Summary`.

use std::fmt::Display;

use serde::Serialize;

pub const CSV_HEADER: &str =
	"t_ms,app,cpu_pct,pss_kb,battery_pct,mah_cum,mah_delta,frames,janky,fps";

/// Everything one tick observed for one app. `None` = metric not
/// available this tick (app idle between samples, first tick, reset) —
/// it renders as an empty CSV cell, never as a fake zero.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Sample {
	pub t_ms: u64,
	pub app: String,
	pub cpu_pct: Option<f64>,
	pub pss_kb: Option<u64>,
	pub battery_pct: Option<u32>,
	pub mah_cum: Option<f64>,
	pub mah_delta: Option<f64>,
	pub frames: Option<u64>,
	pub janky: Option<u64>,
	pub fps: Option<f64>,
}

fn cell<T: Display>(v: &Option<T>) -> String {
	v.as_ref().map(ToString::to_string).unwrap_or_default()
}

impl Sample {
	pub fn csv_row(&self) -> String {
		[
			self.t_ms.to_string(),
			self.app.clone(),
			cell(&self.cpu_pct),
			cell(&self.pss_kb),
			cell(&self.battery_pct),
			cell(&self.mah_cum),
			cell(&self.mah_delta),
			cell(&self.frames),
			cell(&self.janky),
			cell(&self.fps),
		]
		.join(",")
	}
}

/// Aggregates over one app's samples — printed after the CSV, embedded
/// in the JSON document.
#[derive(Debug, Clone, Serialize)]
pub struct Summary {
	pub app: String,
	pub samples: u32,
	/// Ticks where the app was not running (`pidof` empty).
	pub skipped: u32,
	pub avg_cpu_pct: Option<f64>,
	pub max_pss_kb: Option<u64>,
	/// Sum of per-tick mAh deltas — the run's per-app drain.
	pub mah_drained: Option<f64>,
	pub frames: Option<u64>,
	pub janky: Option<u64>,
	pub avg_fps: Option<f64>,
	/// Janky frames over total frames, percent.
	pub jank_pct: Option<f64>,
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
	let mut sum = 0.0;
	let mut n = 0u32;
	for v in values {
		sum += v;
		n += 1;
	}
	(n > 0).then(|| sum / n as f64)
}

pub fn summarize(app: &str, samples: &[Sample], skipped: u32) -> Summary {
	let avg_cpu_pct = mean(samples.iter().filter_map(|s| s.cpu_pct));
	let max_pss_kb = samples.iter().filter_map(|s| s.pss_kb).max();
	let any_mah = samples.iter().any(|s| s.mah_delta.is_some());
	let mah_drained: Option<f64> =
		any_mah.then(|| samples.iter().filter_map(|s| s.mah_delta).sum());
	let any_frames = samples.iter().any(|s| s.frames.is_some());
	let any_janky = samples.iter().any(|s| s.janky.is_some());
	let frames = samples.iter().filter_map(|s| s.frames).sum::<u64>();
	let janky = samples.iter().filter_map(|s| s.janky).sum::<u64>();
	let avg_fps = mean(samples.iter().filter_map(|s| s.fps));
	let jank_pct = (any_frames && frames > 0).then(|| janky as f64 * 100.0 / frames as f64);
	Summary {
		app: app.to_string(),
		samples: samples.len() as u32,
		skipped,
		avg_cpu_pct,
		max_pss_kb,
		mah_drained,
		frames: any_frames.then_some(frames),
		janky: any_janky.then_some(janky),
		avg_fps,
		jank_pct,
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn sample(t_ms: u64) -> Sample {
		Sample {
			t_ms,
			app: "com.example.app".into(),
			cpu_pct: Some(10.0),
			pss_kb: Some(1000),
			battery_pct: Some(87),
			mah_cum: Some(12.5),
			mah_delta: Some(0.5),
			frames: Some(60),
			janky: Some(2),
			fps: Some(59.9),
		}
	}

	#[test]
	fn csv_row_is_comma_separated_with_empty_cells_for_missing_metrics() {
		assert_eq!(sample(1000).csv_row(), "1000,com.example.app,10,1000,87,12.5,0.5,60,2,59.9");
		let mut s = sample(2000);
		s.cpu_pct = None;
		s.fps = None;
		assert_eq!(s.csv_row(), "2000,com.example.app,,1000,87,12.5,0.5,60,2,");
	}

	#[test]
	fn summary_averages_maxes_and_sums_across_samples() {
		let mut b = sample(2000);
		b.cpu_pct = Some(20.0);
		b.pss_kb = Some(3000);
		b.mah_delta = Some(0.25);
		b.frames = Some(40);
		b.janky = Some(10);
		b.fps = Some(30.0);
		let mut c = sample(3000);
		c.cpu_pct = None; // missing ticks must not drag the average to 0
		c.frames = None;
		c.janky = None;
		c.fps = None;
		c.mah_delta = None;
		let summary = summarize("com.example.app", &[sample(1000), b, c], 2);
		assert_eq!(summary.samples, 3);
		assert_eq!(summary.skipped, 2);
		assert_eq!(summary.avg_cpu_pct, Some(15.0));
		assert_eq!(summary.max_pss_kb, Some(3000));
		assert_eq!(summary.mah_drained, Some(0.75));
		assert_eq!(summary.frames, Some(100));
		assert_eq!(summary.janky, Some(12));
		assert_eq!(summary.avg_fps, Some(44.95));
		assert_eq!(summary.jank_pct, Some(12.0));
	}

	#[test]
	fn summary_of_nothing_is_all_none_not_zero() {
		let summary = summarize("com.example.app", &[], 5);
		assert_eq!(summary.samples, 0);
		assert_eq!(summary.skipped, 5);
		assert!(summary.avg_cpu_pct.is_none());
		assert!(summary.max_pss_kb.is_none());
		assert!(summary.mah_drained.is_none());
		assert!(summary.frames.is_none());
		assert!(summary.jank_pct.is_none());
	}
}
