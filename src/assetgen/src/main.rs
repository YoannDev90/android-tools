//! `assetgen` — turn one source PNG into the full Android density set:
//! legacy launcher icons, notification icons, generic drawables, adaptive
//! icons, 9-patch rescales and badged debug variants for `src/debug/res`.
//! All generation lives in `gen`; this file is CLI, filesystem and exit
//! codes.

mod gen;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use image::DynamicImage;

use gen::{Asset, DENSITIES};

#[derive(Parser, Debug)]
#[command(
	name = "assetgen",
	about = "Generate Android density assets from one source PNG",
	version
)]
struct Cli {
	#[command(subcommand)]
	mode: Mode,

	/// Destination res/ directory (created when missing).
	#[arg(short = 'o', long, global = true, default_value = "res")]
	out: PathBuf,

	/// Resource name — [a-z][a-z0-9_]*.
	#[arg(short = 'n', long, global = true, default_value = "ic_launcher")]
	name: String,
}

#[derive(Subcommand, Debug)]
enum Mode {
	/// Legacy launcher icons (mipmap-*, 48 dp); --round adds *_round.
	Launcher {
		/// Square PNG, ideally 192x192 (xxxhdpi).
		#[arg(value_name = "PNG")]
		source: PathBuf,
		/// Square PNG for the round variant (circle-cropped).
		#[arg(long)]
		round: Option<PathBuf>,
	},
	/// Notification icons (drawable-*, 24 dp).
	Notification {
		#[arg(value_name = "PNG")]
		source: PathBuf,
	},
	/// Generic drawable at a dp base of your choice (default 24).
	Drawable {
		#[arg(value_name = "PNG")]
		source: PathBuf,
		#[arg(long, default_value_t = 24)]
		dp: u32,
	},
	/// Adaptive icon: 108 dp foreground + background + v26 entries.
	Adaptive {
		#[arg(value_name = "PNG")]
		source: PathBuf,
		/// Background as a color (#RGB / #RRGGBB / #AARRGGBB).
		#[arg(long, value_name = "COLOR")]
		bg_color: Option<String>,
		/// Background as a square PNG.
		#[arg(long, value_name = "PNG")]
		bg: Option<PathBuf>,
	},
	/// Rescale an existing .9.png across the other densities.
	#[command(name = "9patch")]
	NinePatch {
		#[arg(value_name = "9PNG")]
		source: PathBuf,
		/// Density the source was authored at.
		#[arg(long, default_value = "xhdpi")]
		from: String,
	},
	/// Launcher icons for debug builds: the launcher set plus a badge,
	/// meant for src/debug/res (AGP applies it over src/main/res in
	/// debug builds — no Gradle change needed).
	Debug {
		/// Square PNG, ideally 192x192 (xxxhdpi).
		#[arg(value_name = "PNG")]
		source: PathBuf,
		/// Square PNG for the round variant (circle-cropped, badged too).
		#[arg(long)]
		round: Option<PathBuf>,
		/// Badge artwork: SVG (rendered crisp per density) or square PNG.
		#[arg(long, value_name = "SVG|PNG")]
		badge: PathBuf,
		/// Corner the badge sits in.
		#[arg(long, default_value = "br")]
		badge_corner: CornerArg,
		/// Badge size, percent of the icon.
		#[arg(long, default_value_t = 33)]
		badge_size: u32,
	},
}

/// Where the debug badge sits (`--badge-corner`).
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
enum CornerArg {
	Tl,
	Tr,
	Bl,
	Br,
}

impl From<CornerArg> for gen::Corner {
	fn from(arg: CornerArg) -> Self {
		match arg {
			CornerArg::Tl => gen::Corner::Tl,
			CornerArg::Tr => gen::Corner::Tr,
			CornerArg::Bl => gen::Corner::Bl,
			CornerArg::Br => gen::Corner::Br,
		}
	}
}

fn main() {
	let cli = Cli::parse();
	if let Err(e) = run(&cli) {
		eprintln!("assetgen: {e:#}");
		std::process::exit(2);
	}
}

fn open(path: &Path) -> Result<DynamicImage> {
	image::open(path).with_context(|| format!("open {}", path.display()))
}

/// Asset Studio assumes a source that covers the largest density; smaller
/// sources still work but upscale (and blur) the smaller buckets.
fn warn_upscale(source: u32, target: u32, what: &str) {
	if source < target {
		eprintln!(
			"warning: {what} source is {source}px, below the xxxhdpi target of {target}px — some densities will upscale"
		);
	}
}

/// Badges arrive as an SVG (vector, re-rendered per density) or a square
/// PNG; anything else is refused loudly instead of guessed at.
fn load_badge(path: &Path) -> Result<gen::Badge> {
	match path.extension().and_then(|e| e.to_str()) {
		Some("svg") => {
			let data = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
			Ok(gen::Badge::Svg(data))
		}
		Some("png") => {
			let img = open(path)?.to_rgba8();
			if img.width() != img.height() {
				bail!("badge PNG is {}x{}, expected square", img.width(), img.height());
			}
			Ok(gen::Badge::Png(img))
		}
		other => bail!("badge must be .svg or .png, got {other:?}"),
	}
}

fn run(cli: &Cli) -> Result<()> {
	let assets = match &cli.mode {
		Mode::Launcher { source, round } => {
			let src = open(source)?;
			warn_upscale(src.width(), gen::dp_px(gen::LAUNCHER_DP, 4.0), "launcher");
			let round = round.as_ref().map(|p| open(p)).transpose()?;
			if let Some(round) = &round {
				warn_upscale(round.width(), gen::dp_px(gen::LAUNCHER_DP, 4.0), "round");
			}
			gen::launcher(&src, &cli.name, round.as_ref()).map_err(anyhow::Error::msg)?
		}
		Mode::Notification { source } => {
			let src = open(source)?;
			warn_upscale(src.width(), gen::dp_px(gen::NOTIFICATION_DP, 4.0), "notification");
			gen::notification(&src, &cli.name).map_err(anyhow::Error::msg)?
		}
		Mode::Drawable { source, dp } => {
			let src = open(source)?;
			warn_upscale(src.width(), gen::dp_px(*dp, 4.0), "drawable");
			gen::drawable(&src, &cli.name, *dp).map_err(anyhow::Error::msg)?
		}
		Mode::Adaptive { source, bg_color, bg } => {
			let src = open(source)?;
			warn_upscale(src.width(), gen::dp_px(gen::ADAPTIVE_DP, 4.0), "adaptive");
			let background = match (bg_color, bg) {
				(Some(_), Some(_)) => bail!("--bg-color and --bg are mutually exclusive"),
				(Some(hex), None) => gen::Background::Color(hex.clone()),
				(None, Some(path)) => gen::Background::Image(open(path)?),
				(None, None) => bail!("adaptive needs a background: --bg-color or --bg"),
			};
			gen::adaptive(&src, &cli.name, background).map_err(anyhow::Error::msg)?
		}
		Mode::NinePatch { source, from } => {
			let from_density =
				DENSITIES.iter().find(|&&(name, _)| name == from).map(|&(_, d)| d).with_context(
					|| format!("unknown density {from:?}, use mdpi/hdpi/xhdpi/xxhdpi/xxxhdpi"),
				)?;
			if from_density < 4.0 {
				eprintln!("warning: densities above {from} will upscale this patch");
			}
			let src = open(source)?.to_rgba8();
			gen::nine_patch(&src, &cli.name, from_density).map_err(anyhow::Error::msg)?
		}
		Mode::Debug { source, round, badge, badge_corner, badge_size } => {
			let src = open(source)?;
			warn_upscale(src.width(), gen::dp_px(gen::LAUNCHER_DP, 4.0), "debug launcher");
			let round = round.as_ref().map(|p| open(p)).transpose()?;
			if let Some(round) = &round {
				warn_upscale(round.width(), gen::dp_px(gen::LAUNCHER_DP, 4.0), "debug round");
			}
			let badge = load_badge(badge)?;
			gen::debug(&src, &cli.name, round.as_ref(), &badge, (*badge_corner).into(), *badge_size)
				.map_err(anyhow::Error::msg)?
		}
	};
	for asset in &assets {
		write(cli.out.join(&asset.dir), asset)?;
	}
	Ok(())
}

fn write(dir: PathBuf, asset: &Asset) -> Result<()> {
	std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
	let path = dir.join(&asset.file);
	if let Some(img) = &asset.image {
		img.save(&path).with_context(|| format!("write {}", path.display()))?;
		println!("{} {}x{}", path.display(), img.width(), img.height());
	} else if let Some(text) = &asset.text {
		std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
		println!("{}", path.display());
	}
	Ok(())
}
