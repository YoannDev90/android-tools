//! `startperf` — cold/warm app start benchmark over `adb`.
//!
//! Measures `am start -W` timings over N runs and reports min / median / mean /
//! max, as a table (TTY), JSON or CSV. No Android Studio involved.

mod output;
mod parse;
mod run;
mod stats;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};

use adbkit::{pick_serial, resolve_launcher, Adb};

use run::{benchmark, AdbDevice, Mode};
use stats::summarize;

#[derive(Parser, Debug)]
#[command(
	name = "startperf",
	about = "App cold/warm start benchmark — min/median/mean/max over N runs",
	version
)]
struct Cli {
	/// App package name (launcher activity is resolved automatically).
	#[arg(short, long)]
	package: Option<String>,

	/// Full component `pkg/.Activity` — skips launcher resolution.
	#[arg(short = 'n', long)]
	component: Option<String>,

	/// adb serial (auto-picked when a single device is connected).
	#[arg(short, long, global = true)]
	serial: Option<String>,

	/// Measured runs (a skipped run does not count).
	#[arg(short, long, default_value_t = 5)]
	runs: u32,

	/// Which kind of start to measure.
	#[arg(long, value_enum)]
	mode: Option<ModeArg>,

	/// Measure cold start (force-stop before each run). Default.
	#[arg(long, conflicts_with = "warm")]
	cold: bool,

	/// Measure warm start: HOME before each run (process kept alive, activity
	/// recreated).
	#[arg(long, conflicts_with = "cold")]
	warm: bool,

	/// Pause in ms after each action (device settling).
	#[arg(long, default_value_t = 500)]
	settle: u64,

	/// Write samples as CSV to this path (also prints the table).
	#[arg(long)]
	csv: Option<PathBuf>,

	/// Print the full report as JSON instead of a table.
	#[arg(long)]
	json: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum ModeArg {
	Cold,
	Warm,
}

impl From<ModeArg> for Mode {
	fn from(m: ModeArg) -> Self {
		match m {
			ModeArg::Cold => Mode::Cold,
			ModeArg::Warm => Mode::Warm,
		}
	}
}

fn main() -> Result<()> {
	let cli = Cli::parse();

	// `--mode` is the general form; `--cold`/`--warm` are its common shorthands.
	let mode = match (cli.mode, cli.cold, cli.warm) {
		(Some(m), _, _) => Mode::from(m),
		(_, true, _) => Mode::Cold,
		(_, _, true) => Mode::Warm,
		_ => Mode::Cold,
	};

	let component = match &cli.component {
		Some(c) => c.clone(),
		None => {
			let pkg = cli
				.package
				.as_deref()
				.context("pass --package <pkg> or --component <pkg/.Activity>")?;
			let serial = pick_serial(cli.serial.as_deref())?;
			let dev = Adb::new(&serial);
			resolve_launcher(&dev, pkg)?
		}
	};
	// Force-stopping needs the package, which a bare component may not spell.
	let package = cli
		.package
		.clone()
		.unwrap_or_else(|| component.split('/').next().unwrap_or_default().to_string());

	if cli.runs == 0 {
		bail!("--runs must be at least 1");
	}

	let serial = pick_serial(cli.serial.as_deref())?;
	eprintln!("startperf: {mode:?} start of {component} on {serial} × {} run(s)", cli.runs);

	let mut dev = AdbDevice { dev: &Adb::new(&serial) };
	let outcome = benchmark(&mut dev, &package, &component, cli.runs, mode, cli.settle)?;

	for (run, reason) in &outcome.skipped {
		eprintln!("  run {run} skipped: {reason}");
	}
	if outcome.samples.is_empty() {
		bail!(
			"no successful run ({} skipped) — is the app installed, and is the component right?",
			outcome.skipped.len()
		);
	}

	let stats = summarize(&outcome.samples.iter().map(|s| s.total_ms).collect::<Vec<_>>())?;

	if cli.json {
		output::print_json(mode, &package, &component, cli.runs, &outcome, &stats)?;
	} else {
		output::print_table(mode, &component, &outcome, &stats);
	}

	if let Some(path) = &cli.csv {
		output::write_csv(path, mode, &outcome)
			.with_context(|| format!("write {}", path.display()))?;
		eprintln!("csv: {} written", path.display());
	}

	// Non-zero exit when nothing was measured — already bailed above — but a
	// wholly skipped set is also a failed benchmark.
	Ok(())
}
