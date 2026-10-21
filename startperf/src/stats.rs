//! Summary statistics over the measured runs (min / median / mean / max).
//!
//! Median is reported rather than derived from the mean because cold-start
//! samples are typically skewed by one GC/JIT hiccup: mean alone overstates
//! the typical run.

use anyhow::{bail, Result};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Stats {
	pub n: usize,
	pub min: u64,
	pub median: f64,
	pub mean: f64,
	pub max: u64,
	/// Sample standard deviation (n-1); 0.0 for a single run.
	pub stddev: f64,
}

/// Summarize a set of millisecond samples. Empty input is a caller bug.
pub fn summarize(samples: &[u64]) -> Result<Stats> {
	if samples.is_empty() {
		bail!("no successful runs to summarize");
	}
	let mut sorted = samples.to_vec();
	sorted.sort_unstable();

	let n = sorted.len();
	let min = sorted[0];
	let max = sorted[n - 1];
	let sum: u128 = samples.iter().map(|&v| v as u128).sum();
	let mean = sum as f64 / n as f64;

	let median = if n % 2 == 1 {
		sorted[n / 2] as f64
	} else {
		(sorted[n / 2 - 1] + sorted[n / 2]) as f64 / 2.0
	};

	let stddev = if n < 2 {
		0.0
	} else {
		let var = samples
			.iter()
			.map(|&v| {
				let d = v as f64 - mean;
				d * d
			})
			.sum::<f64>()
			/ (n - 1) as f64;
		var.sqrt()
	};

	Ok(Stats { n, min, median, mean, max, stddev })
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn odd_count_median_is_middle_value() {
		let s = summarize(&[800, 900, 1000]).unwrap();
		assert_eq!(s.n, 3);
		assert_eq!(s.min, 800);
		assert_eq!(s.median, 900.0);
		assert_eq!(s.mean, 900.0);
		assert_eq!(s.max, 1000);
		assert!((s.stddev - 100.0).abs() < 1e-9);
	}

	#[test]
	fn even_count_median_is_average_of_middle_pair() {
		let s = summarize(&[100, 200, 300, 400]).unwrap();
		assert_eq!(s.median, 250.0);
		assert_eq!(s.mean, 250.0);
		assert_eq!((s.min, s.max), (100, 400));
	}

	#[test]
	fn median_ignores_outlier_that_mean_absorbs() {
		// One GC hiccup: mean is dragged, median stays representative.
		let s = summarize(&[800, 810, 820, 830, 2000]).unwrap();
		assert_eq!(s.median, 820.0);
		assert!((s.mean - 1052.0).abs() < 1e-9, "{}", s.mean);
		assert_eq!(s.max, 2000);
	}

	#[test]
	fn single_sample_has_zero_stddev() {
		let s = summarize(&[842]).unwrap();
		assert_eq!((s.n, s.min, s.max), (1, 842, 842));
		assert_eq!(s.median, 842.0);
		assert_eq!(s.mean, 842.0);
		assert_eq!(s.stddev, 0.0);
	}

	#[test]
	fn unsorted_input_is_handled() {
		let s = summarize(&[300, 100, 200]).unwrap();
		assert_eq!((s.min, s.median, s.max), (100, 200.0, 300));
	}

	#[test]
	fn empty_is_an_error() {
		assert!(summarize(&[]).is_err());
	}

	#[test]
	fn serializes_to_json() {
		let s = summarize(&[1, 2]).unwrap();
		let v: serde_json::Value = serde_json::to_value(s).unwrap();
		assert_eq!(v["n"], 2);
		assert_eq!(v["median"], 1.5);
	}
}
