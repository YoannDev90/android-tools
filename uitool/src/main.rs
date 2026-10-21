mod adb;
mod shot;
mod ui;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use adbkit::{pick_serial, Adb};

use shot::ShotOpts;

#[derive(Parser, Debug)]
#[command(name = "uitool", about = "Android UI driver — dump, find, tap, shot", version)]
struct Cli {
	/// adb serial (auto-picked when a single device is connected).
	#[arg(short, long, global = true)]
	serial: Option<String>,

	#[command(subcommand)]
	cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
	/// Wake + dismiss keyguard + collapse status bar.
	Wake,
	/// Dump UI hierarchy: one line per labeled node (`text [bounds]`).
	Dump,
	/// Find nodes matching text/content-desc (case-insensitive substring). Exit 1 if none.
	Find {
		/// Substring to search.
		query: String,
		/// Only clickable nodes.
		#[arg(long)]
		clickable: bool,
	},
	/// Wait until a node matches (poll dump).
	Wait {
		query: String,
		/// Timeout seconds.
		#[arg(short = 't', long, default_value_t = 10)]
		timeout: u64,
	},
	/// Tap by coordinates (`uitool tap 978 2316`) or by label (`uitool tap Aperçu`).
	Tap {
		/// Either `x y` (two args) or a label.
		args: Vec<String>,
		/// Milliseconds to hold the press (0 = quick `input tap`).
		#[arg(short, long, default_value_t = 0)]
		duration: u64,
	},
	/// Long-press a label.
	LongPress {
		query: String,
		#[arg(short, long, default_value_t = 600)]
		duration: u64,
	},
	/// Swipe: uitool swipe 540 2000 540 800 [ms]
	Swipe {
		x1: i32,
		y1: i32,
		x2: i32,
		y2: i32,
		#[arg(default_value_t = 300)]
		ms: u64,
	},
	/// Screenshot → jpg/png on disk (prints path). Defaults /tmp/uitool-*.jpg.
	Shot {
		/// Output path (.jpg/.png). Default: /tmp/uitool-<epoch>.jpg
		#[arg(short, long)]
		out: Option<PathBuf>,
		/// Crop `x1,y1,x2,y2` before save.
		#[arg(long)]
		crop: Option<String>,
		/// Nearest upscale factor (after crop), e.g. 2.
		#[arg(long, default_value_t = 1.0)]
		scale: f32,
		/// Cap longest side (0 = keep full).
		#[arg(long, default_value_t = 0)]
		max_dim: u32,
		/// JPEG quality 1-100.
		#[arg(long, default_value_t = 75)]
		quality: u8,
		/// Wake before capture.
		#[arg(long)]
		wake: bool,
	},
	/// Launch an activity: package (resolves its launcher activity) or package/.Activity.
	Launch {
		/// Package name, or package/component.
		target: String,
	},
	/// Send key event (KEYCODE_HOME or numeric).
	Key { code: String },
	/// Type text (spaces → %s).
	Input { text: String },
	/// Force-stop a package.
	Stop { package: String },
}

fn main() -> Result<()> {
	let cli = Cli::parse();
	let serial = pick_serial(cli.serial.as_deref())?;
	let dev = Adb::new(&serial);

	match cli.cmd {
		Cmd::Wake => {
			adb::wake(&dev)?;
			println!("ok");
		}
		Cmd::Dump => {
			let xml = ui::dump(&dev)?;
			let nodes = ui::parse_hierarchy(&xml);
			// write! so `| head` exits 0 instead of panicking on SIGPIPE.
			let mut out = std::io::stdout().lock();
			for n in nodes {
				let b = n
					.bounds
					.map(|b| format!("[{},{},{},{}]", b.x1, b.y1, b.x2, b.y2))
					.unwrap_or_else(|| "?".into());
				let click = if n.clickable { " *" } else { "" };
				let line = format!("{} {}{}", n.label().replace('\n', "\\n"), b, click);
				if std::io::Write::write_fmt(&mut out, format_args!("{line}\n")).is_err() {
					break;
				}
			}
		}
		Cmd::Find { query, clickable } => {
			let xml = ui::dump(&dev)?;
			let nodes = ui::parse_hierarchy(&xml);
			let mut found = ui::find_nodes(&nodes, &query);
			if clickable {
				found.retain(|n| n.clickable);
			}
			if found.is_empty() {
				bail!("no node matching {query:?}");
			}
			for n in found {
				if let Some(b) = n.bounds {
					println!(
						"[{},{},{},{}] cx={} cy={} clickable={}",
						b.x1,
						b.y1,
						b.x2,
						b.y2,
						b.cx(),
						b.cy(),
						n.clickable
					);
				}
			}
		}
		Cmd::Wait { query, timeout } => {
			let deadline = Instant::now() + Duration::from_secs(timeout);
			loop {
				let xml = ui::dump(&dev)?;
				let nodes = ui::parse_hierarchy(&xml);
				if let Some(n) = ui::find_nodes(&nodes, &query).first() {
					if let Some(b) = n.bounds {
						println!("[{},{},{},{}]", b.x1, b.y1, b.x2, b.y2);
						return Ok(());
					}
				}
				if Instant::now() >= deadline {
					bail!("timeout waiting for {query:?}");
				}
				std::thread::sleep(Duration::from_millis(300));
			}
		}
		Cmd::Tap { args, duration } => match args.len() {
			2 => {
				let x: i32 = args[0].parse().context("x")?;
				let y: i32 = args[1].parse().context("y")?;
				adb::press(&dev, x, y, duration)?;
				println!("{x},{y}");
			}
			1 => {
				let xml = ui::dump(&dev)?;
				let nodes = ui::parse_hierarchy(&xml);
				let n = ui::pick_tap_target(&nodes, &args[0])?;
				let b = n.bounds.unwrap();
				adb::press(&dev, b.cx(), b.cy(), duration)?;
				println!("{},{} ({})", b.cx(), b.cy(), args[0]);
			}
			_ => bail!("usage: uitool tap <x> <y> | uitool tap <label>"),
		},
		Cmd::LongPress { query, duration } => {
			let xml = ui::dump(&dev)?;
			let nodes = ui::parse_hierarchy(&xml);
			let n = ui::pick_tap_target(&nodes, &query)?;
			let b = n.bounds.unwrap();
			adb::press(&dev, b.cx(), b.cy(), duration)?;
			println!("{},{} ({})", b.cx(), b.cy(), query);
		}
		Cmd::Swipe { x1, y1, x2, y2, ms } => {
			dev.shell(&[
				"input",
				"swipe",
				&x1.to_string(),
				&y1.to_string(),
				&x2.to_string(),
				&y2.to_string(),
				&ms.to_string(),
			])?;
			println!("{x1},{y1} -> {x2},{y2}");
		}
		Cmd::Shot { out, crop, scale, max_dim, quality, wake } => {
			let crop = match crop {
				Some(c) => {
					let v: Vec<i32> = c
						.split(',')
						.map(|s| s.trim().parse().context("crop must be x1,y1,x2,y2"))
						.collect::<Result<Vec<_>, _>>()?;
					if v.len() != 4 {
						bail!("crop must be x1,y1,x2,y2");
					}
					Some([v[0], v[1], v[2], v[3]])
				}
				None => None,
			};
			let opts = ShotOpts { out, crop, scale, max_dim, quality, wake };
			let path = shot::take(&dev, &opts)?;
			println!("{}", path.display());
		}
		Cmd::Launch { target } => {
			let component =
				if target.contains('/') { target } else { adb::resolve_launcher(&dev, &target)? };
			dev.shell(&["am", "start", "-n", &component])?;
			println!("started {component}");
		}
		Cmd::Key { code } => {
			let c = if code.chars().all(|c| c.is_ascii_digit()) {
				code
			} else {
				format!("KEYCODE_{code}")
			};
			dev.shell(&["input", "keyevent", &c])?;
			println!("{c}");
		}
		Cmd::Input { text } => {
			// adb input text: spaces as %s, escape shell specials lightly.
			let escaped = text
				.replace('\\', "\\\\")
				.replace(' ', "%s")
				.replace('"', "\\\"")
				.replace("'", "%s");
			dev.shell(&["input", "text", &escaped])?;
			println!("sent {} bytes", text.len());
		}
		Cmd::Stop { package } => {
			dev.shell(&["am", "force-stop", &package])?;
			println!("stopped {package}");
		}
	}
	Ok(())
}
