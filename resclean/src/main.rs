//! CLI: point at an Android project, get its dead `res/` resources — text
//! or JSON, and an exit code CI can gate on. The analysis itself is
//! `scan`.

mod scan;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use regex::Regex;

use scan::{scan, Report};

#[derive(Parser, Debug)]
#[command(name = "resclean", about = "Dead res/ resource detector", version)]
struct Cli {
	/// Project (or module) root to scan.
	#[arg(default_value = ".")]
	path: PathBuf,

	/// Keep resources whose `kind/name` matches this regex (repeatable) —
	/// e.g. `--keep 'string/get'` for getIdentifier() lookups.
	#[arg(long)]
	keep: Vec<String>,

	/// Machine-readable report on stdout.
	#[arg(long)]
	json: bool,

	/// Report the dead resources but always exit 0.
	#[arg(long)]
	warn_only: bool,
}

fn main() {
	let cli = Cli::parse();
	match run(&cli) {
		// 0: nothing dead — 1: dead resources (the CI gate) — 2: error.
		Ok(code) => std::process::exit(code),
		Err(e) => {
			eprintln!("resclean: {e:#}");
			std::process::exit(2);
		}
	}
}

fn run(cli: &Cli) -> Result<i32> {
	let keep: Vec<Regex> = cli
		.keep
		.iter()
		.map(|p| Regex::new(p).with_context(|| format!("bad --keep pattern {p:?}")))
		.collect::<Result<_>>()?;

	let report = scan(&cli.path, &keep)?;
	if cli.json {
		print_json(&report)?;
	} else {
		print_text(&report);
	}
	Ok(i32::from(report.total() > 0 && !cli.warn_only))
}

fn print_text(report: &Report) {
	if report.total() == 0 {
		println!("clean — {} resources defined, all referenced", report.defs);
		return;
	}
	println!(
		"{} dead resource{} ({} defined, {} referenced)",
		report.total(),
		if report.total() == 1 { "" } else { "s" },
		report.defs,
		report.refs
	);
	for (kind, names) in &report.dead {
		println!("{kind}/ ({}):", names.len());
		for name in names {
			println!("  {kind}/{name}");
		}
	}
}

fn print_json(report: &Report) -> Result<()> {
	#[derive(serde::Serialize)]
	struct JsonReport<'a> {
		dead: &'a std::collections::BTreeMap<String, Vec<String>>,
		defined: usize,
		referenced: usize,
		total: usize,
		failed: bool,
	}
	let out = JsonReport {
		dead: &report.dead,
		defined: report.defs,
		referenced: report.refs,
		total: report.total(),
		failed: report.total() > 0,
	};
	println!("{}", serde_json::to_string_pretty(&out)?);
	Ok(())
}
