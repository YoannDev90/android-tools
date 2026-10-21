//! CLI: a uiautomator dump in — a saved file (`--xml`, no device needed,
//! CI replay) or the connected device (`--device`) — findings and an exit
//! code out. The rules live in `rules`, the device access in `device`.

mod device;
mod rules;

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;

use adbkit::{pick_serial, Adb};

use rules::{lint, Opts};

#[derive(Parser, Debug)]
#[command(
	name = "layoutlint",
	about = "UI hierarchy lint — touch targets, overlap, labels",
	version
)]
struct Cli {
	/// Lint a saved uiautomator dump; no device needed (CI replay).
	#[arg(long, conflicts_with = "device")]
	xml: Option<PathBuf>,

	/// Dump the connected device, then lint it.
	#[arg(long)]
	device: bool,

	/// adb serial (auto-picked when exactly one device is connected).
	#[arg(short, long)]
	serial: Option<String>,

	/// Screen density in dpi for px→dp. Default: the device's in --device
	/// mode (override density when set), 160 otherwise.
	#[arg(long)]
	density: Option<u32>,

	/// Minimum touch target in dp (Android's accessibility guidance: 48).
	#[arg(long, default_value_t = 48)]
	min_touch: u32,

	/// Machine-readable report on stdout.
	#[arg(long)]
	json: bool,

	/// Report every finding but always exit 0.
	#[arg(long)]
	warn_only: bool,
}

fn main() {
	let cli = Cli::parse();
	match run(&cli) {
		// Exit codes mirror scenediff's contract: 0 clean, 1 findings (the
		// CI gate), 2 for any error — a broken run shares neither 0 (which
		// would read as a clean lint) nor 1.
		Ok(code) => std::process::exit(code),
		Err(e) => {
			eprintln!("layoutlint: {e:#}");
			std::process::exit(2);
		}
	}
}

fn run(cli: &Cli) -> Result<i32> {
	let (xml, density) = if cli.device {
		let serial = pick_serial(cli.serial.as_deref())?;
		let dev = Adb::new(&serial);
		let density = match cli.density {
			Some(d) => d,
			None => device::device_density(&dev)?,
		};
		(device::dump(&dev)?, density)
	} else {
		let Some(path) = &cli.xml else {
			bail!("pass --xml <dump> or --device");
		};
		let xml =
			std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
		(xml, cli.density.unwrap_or(160))
	};

	let nodes = adbkit::hierarchy::parse_hierarchy(&xml);
	let opts = Opts { min_touch: cli.min_touch, density };
	let findings = lint(&nodes, &opts);

	// Any finding fails, with or without --warn-only: severity orders the
	// report, it does not decide the gate.
	let failed = !findings.is_empty() && !cli.warn_only;
	if cli.json {
		println!("{}", rules::render_json(&nodes, &opts, &findings, failed)?);
	} else {
		println!("{}", rules::render_text(&nodes, &opts, &findings));
	}
	Ok(i32::from(failed))
}
