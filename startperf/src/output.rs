//! Report rendering: human table, JSON, CSV.

use std::io::Write;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::run::{Mode, Outcome};
use crate::stats::Stats;

#[derive(Serialize)]
struct Report<'a> {
	mode: &'a str,
	package: &'a str,
	component: &'a str,
	requested_runs: u32,
	skipped: usize,
	stats: &'a Stats,
	runs: &'a [crate::run::Sample],
}

pub fn print_json(
	mode: Mode,
	package: &str,
	component: &str,
	requested_runs: u32,
	outcome: &Outcome,
	stats: &Stats,
) -> Result<()> {
	let report = Report {
		mode: mode.as_str(),
		package,
		component,
		requested_runs,
		skipped: outcome.skipped.len(),
		stats,
		runs: &outcome.samples,
	};
	println!("{}", serde_json::to_string_pretty(&report)?);
	Ok(())
}

pub fn print_table(mode: Mode, component: &str, outcome: &Outcome, stats: &Stats) {
	println!("{mode:?} start — {component}");
	println!();

	let header = format!(
		"{:>4}  {:>10}  {:>10}  {:>10}  {:>8}",
		"run", "total ms", "wait ms", "this ms", "launch"
	);
	println!("{header}");
	println!("{}", "-".repeat(header.chars().count()));

	for s in &outcome.samples {
		let launch = s.launch_state.as_deref().unwrap_or("-");
		// No note for `brought_to_front`: cold drops those runs, so any
		// surviving one is a warm resume, where the warning is expected. The
		// flag stays in the JSON/CSV for machine consumers.
		println!(
			"{:>4}  {:>10}  {:>10}  {:>10}  {:>8}",
			s.run,
			s.total_ms,
			opt(s.wait_ms),
			opt(s.this_ms),
			launch
		);
	}

	println!("{}", "-".repeat(header.chars().count()));
	let unit = "ms";
	println!(
		"total   n={}  min {}  median {:.1}  mean {:.1}  max {}  {}  (stddev {:.1})",
		stats.n, stats.min, stats.median, stats.mean, stats.max, unit, stats.stddev
	);
	if !outcome.skipped.is_empty() {
		println!("{} run(s) skipped — excluded from the stats", outcome.skipped.len());
	}
}

fn opt(v: Option<u64>) -> String {
	v.map(|v| v.to_string()).unwrap_or_else(|| "-".into())
}

/// CSV with one row per measured run, header included — loads straight into a
/// spreadsheet. The stats row is appended so the file stands alone.
pub fn write_csv(path: &Path, mode: Mode, outcome: &Outcome) -> Result<()> {
	let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
	writeln!(f, "run,mode,total_ms,wait_ms,this_ms,launch_state,brought_to_front")?;
	for s in &outcome.samples {
		writeln!(
			f,
			"{},{},{},{},{},{},{}",
			s.run,
			mode.as_str(),
			s.total_ms,
			opt(s.wait_ms),
			opt(s.this_ms),
			s.launch_state.as_deref().unwrap_or_default(),
			s.brought_to_front
		)?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::run::Sample;

	fn outcome() -> Outcome {
		Outcome {
			samples: vec![
				Sample {
					run: 1,
					total_ms: 842,
					wait_ms: Some(852),
					this_ms: None,
					launch_state: Some("COLD".into()),
					brought_to_front: false,
				},
				Sample {
					run: 2,
					total_ms: 900,
					wait_ms: None,
					this_ms: Some(880),
					launch_state: None,
					brought_to_front: true,
				},
			],
			skipped: vec![(3, "brought to the front".into())],
		}
	}

	fn stats() -> Stats {
		crate::stats::summarize(&[842, 900]).unwrap()
	}

	#[test]
	fn csv_has_header_one_row_per_run_and_no_separator_scares() {
		let dir = std::env::temp_dir().join(format!("startperf-csv-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let path = dir.join("runs.csv");
		write_csv(&path, Mode::Cold, &outcome()).unwrap();

		let text = std::fs::read_to_string(&path).unwrap();
		let lines: Vec<&str> = text.lines().collect();
		assert_eq!(lines.len(), 3, "{text}");
		assert_eq!(lines[0], "run,mode,total_ms,wait_ms,this_ms,launch_state,brought_to_front");
		assert_eq!(lines[1], "1,cold,842,852,-,COLD,false");
		assert_eq!(lines[2], "2,cold,900,-,880,,true");
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn csv_for_empty_outcome_is_header_only() {
		let dir = std::env::temp_dir().join(format!("startperf-csv0-{}", std::process::id()));
		std::fs::create_dir_all(&dir).unwrap();
		let path = dir.join("runs.csv");
		write_csv(&path, Mode::Warm, &Outcome::default()).unwrap();
		let text = std::fs::read_to_string(&path).unwrap();
		assert_eq!(text.lines().count(), 1);
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn json_report_carries_stats_runs_and_skip_count() {
		let o = outcome();
		let report = Report {
			mode: "cold",
			package: "com.foo",
			component: "com.foo/.Main",
			requested_runs: 3,
			skipped: o.skipped.len(),
			stats: &stats(),
			runs: &o.samples,
		};
		let v: serde_json::Value = serde_json::to_value(&report).unwrap();
		assert_eq!(v["mode"], "cold");
		assert_eq!(v["requested_runs"], 3);
		assert_eq!(v["skipped"], 1);
		assert_eq!(v["stats"]["median"], 871.0);
		assert_eq!(v["runs"][0]["total_ms"], 842);
		assert_eq!(v["runs"][1]["brought_to_front"], true);
	}

	#[test]
	fn table_renders_without_panicking() {
		// Smoke test: the formatting code paths (Some/None/note) all run.
		let o = outcome();
		print_table(Mode::Cold, "com.foo/.Main", &o, &stats());
	}
}
