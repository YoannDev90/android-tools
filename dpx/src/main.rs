//! `dpx` — px ↔ dp ↔ sp from the command line.
//!
//! Offline by default (any density number works, no device needed);
//! `--device` reads the real one from `wm density`, override winning
//! over physical — that is what the UI actually draws at.

mod convert;

use anyhow::Result;
use clap::Parser;

use adbkit::{device_density, pick_serial, Adb};

use convert::{convert, fmt, Unit};

#[derive(Parser, Debug)]
#[command(
	name = "dpx",
	about = "px ↔ dp ↔ sp converter — offline or against the device's density",
	version
)]
struct Cli {
	/// Value(s) to convert; one result per line.
	#[arg(required = true, num_args = 1.., value_name = "VALUE")]
	values: Vec<f64>,

	/// Unit the values are expressed in.
	#[arg(long, value_enum, default_value_t = Unit::Px)]
	from: Unit,

	/// Unit to convert them to.
	#[arg(long, value_enum, default_value_t = Unit::Dp)]
	to: Unit,

	/// Screen density in dpi (160 = mdpi). Ignored with --device.
	#[arg(short, long, default_value_t = 160, conflicts_with = "device")]
	density: u32,

	/// Read the density from the connected device (`wm density`).
	#[arg(long)]
	device: bool,

	/// Font scale for sp conversions (the device's setting; 1.0 = default).
	#[arg(long, default_value_t = 1.0)]
	font_scale: f64,

	/// adb serial (auto-picked when a single device is connected).
	#[arg(short, long, global = true)]
	serial: Option<String>,
}

fn main() -> Result<()> {
	let cli = Cli::parse();
	let density = if cli.device {
		let serial = pick_serial(cli.serial.as_deref())?;
		device_density(&Adb::new(serial))?
	} else {
		cli.density
	};
	for v in &cli.values {
		println!("{}", fmt(convert(*v, cli.from, cli.to, density, cli.font_scale)?));
	}
	Ok(())
}
