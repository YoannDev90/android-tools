//! CLI: two images in (files, or two live captures), a report and an exit
//! code out. The comparison itself is [`diff`], the rendering [`output`].

mod diff;
mod output;

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::Parser;
use image::RgbaImage;

use adbkit::{pick_serial, Adb};

use output::Report;

#[derive(Parser, Debug)]
#[command(name = "scenediff", about = "Visual regression diff of two screenshots", version)]
struct Cli {
	/// First image (A). Use --device instead to capture from the device.
	a: Option<PathBuf>,
	/// Second image (B).
	b: Option<PathBuf>,

	/// Capture twice from the device instead of reading files.
	#[arg(long)]
	device: bool,

	/// Milliseconds between the two device captures.
	#[arg(long, default_value_t = 1000)]
	delay: u64,

	/// adb serial (auto-picked when a single device is connected).
	#[arg(short, long)]
	serial: Option<String>,

	/// Write the overlay image here (A, red where it differs from B).
	#[arg(short, long)]
	out: Option<PathBuf>,

	/// Exit 1 when the differing percentage is strictly above this.
	#[arg(long, default_value_t = 0.0)]
	threshold: f64,

	/// Compare a crop only: x1,y1,x2,y2 (ignores, e.g., the status bar).
	#[arg(long)]
	region: Option<String>,

	/// Machine-readable report on stdout.
	#[arg(long)]
	json: bool,
}

fn main() {
	let cli = Cli::parse();
	match run(&cli) {
		// Exit codes are the contract: 0 within threshold, 1 above it (the
		// CI gate), 2 for any error — a broken command must never read as a
		// visual regression, so errors cannot share exit 1.
		Ok(code) => std::process::exit(code),
		Err(e) => {
			eprintln!("scenediff: {e:#}");
			std::process::exit(2);
		}
	}
}

fn run(cli: &Cli) -> Result<i32> {
	let (a, b, la, lb) = if cli.device {
		if cli.a.is_some() || cli.b.is_some() {
			bail!("--device takes no image paths");
		}
		let serial = pick_serial(cli.serial.as_deref())?;
		let dev = Adb::new(&serial);
		let a = capture(&dev).context("capture A")?;
		std::thread::sleep(Duration::from_millis(cli.delay));
		let b = capture(&dev).context("capture B")?;
		(a, b, "device".to_string(), format!("device +{}ms", cli.delay))
	} else {
		let (Some(a_path), Some(b_path)) = (&cli.a, &cli.b) else {
			bail!("pass two image paths, or --device");
		};
		let a = load(a_path)?;
		let b = load(b_path)?;
		(a, b, a_path.display().to_string(), b_path.display().to_string())
	};

	// Same region on both sides keeps the compared areas equal by
	// construction, whatever the clamping did.
	let (a, b) = match &cli.region {
		Some(r) => {
			let region = diff::parse_region(r)?;
			(diff::crop(&a, region)?, diff::crop(&b, region)?)
		}
		None => (a, b),
	};

	let d = diff::diff(&a, &b)?;
	if let Some(out) = &cli.out {
		diff::overlay(&a, &b, &d).save(out).with_context(|| format!("write {}", out.display()))?;
	}

	let report = Report::new(&la, &lb, &d, cli.out.as_deref(), cli.threshold);
	if cli.json {
		println!("{}", report.to_json()?);
	} else {
		println!("{}", report.to_text());
	}
	Ok(i32::from(report.failed))
}

/// One live capture as RGBA pixels.
fn capture(dev: &Adb) -> Result<RgbaImage> {
	let png = dev.exec_out(&["screencap", "-p"])?;
	Ok(image::load_from_memory(&png).context("decode screencap PNG")?.to_rgba8())
}

fn load(path: &Path) -> Result<RgbaImage> {
	Ok(image::open(path).with_context(|| format!("open {}", path.display()))?.to_rgba8())
}
