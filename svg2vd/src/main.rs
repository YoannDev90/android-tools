//! `svg2vd` — an SVG in, an Android VectorDrawable XML out. Pure
//! conversion: no device, no Android SDK, nothing but bytes.

mod vd;

use std::io::Read;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;

use vd::Converted;

#[derive(Parser, Debug)]
#[command(name = "svg2vd", about = "Convert an SVG into an Android VectorDrawable XML", version)]
struct Cli {
	/// SVG file, or - for stdin.
	input: PathBuf,

	/// Output file; defaults to the input with a .xml extension, - for
	/// stdout.
	#[arg(short, long)]
	output: Option<PathBuf>,
}

fn main() -> Result<()> {
	let cli = Cli::parse();
	let svg = read_input(&cli.input)?;
	let Converted { xml, warnings } = vd::convert(&svg)?;
	for w in &warnings {
		eprintln!("warning: {w}");
	}
	match cli.output.as_deref() {
		Some(p) if p == PathBuf::from("-").as_path() => print!("{xml}"),
		Some(p) => {
			std::fs::write(p, &xml).with_context(|| format!("write {}", p.display()))?;
			eprintln!("wrote {}", p.display());
		}
		None => {
			let mut out = cli.input.clone();
			out.set_extension("xml");
			std::fs::write(&out, &xml).with_context(|| format!("write {}", out.display()))?;
			eprintln!("wrote {}", out.display());
		}
	}
	Ok(())
}

/// The SVG document, from a file or stdin (`-`).
fn read_input(input: &PathBuf) -> Result<String> {
	if input == &PathBuf::from("-") {
		let mut svg = String::new();
		std::io::stdin().read_to_string(&mut svg).context("read stdin (expected SVG)")?;
		if svg.is_empty() {
			bail!("empty SVG on stdin");
		}
		return Ok(svg);
	}
	std::fs::read_to_string(input)
		.with_context(|| format!("read {} (expected SVG)", input.display()))
}
