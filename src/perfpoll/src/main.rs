//! `perfpoll` — sample per-app CPU, RAM, battery drain and frame rate
//! over adb on a fixed interval, stream rows to CSV (or emit one JSON
//! document at the end). All parsing lives in `parse`, row and summary
//! math in `output`.

mod output;
mod parse;

use std::io::{self, BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;

use adbkit::{pick_serial, Adb};

use output::{summarize, Sample, Summary, CSV_HEADER};
use parse::{
	cpu_pct, delta, parse_battery_level, parse_batterystats_mah, parse_gfxinfo,
	parse_meminfo_pss_kb, parse_pid_ticks, parse_pids, parse_proc_stat_total, parse_user_id,
	per_sec, uid_string,
};

#[derive(Parser, Debug)]
#[command(
	name = "perfpoll",
	about = "Per-app perf sampler: CPU, RAM, battery drain, fps — CSV or JSON",
	version
)]
struct Cli {
	/// Package(s) to sample, repeatable.
	#[arg(short = 'p', long = "package", required = true)]
	packages: Vec<String>,

	/// Sampling interval, milliseconds.
	#[arg(short = 'i', long, default_value_t = 1000)]
	interval_ms: u64,

	/// Run duration, seconds — `0` runs until interrupted.
	#[arg(short = 'd', long, default_value_t = 30)]
	duration_s: u64,

	/// CSV/JSON destination (stdout when omitted).
	#[arg(short = 'o', long)]
	out: Option<PathBuf>,

	/// One JSON document (samples + summary) instead of streamed CSV.
	#[arg(long)]
	json: bool,

	/// adb serial (auto-picked when exactly one device is connected).
	#[arg(long)]
	serial: Option<String>,
}

/// Per-app state between ticks — the previous counters every delta is
/// derived from.
struct AppCtx {
	/// `[package, u0a123]` — how `batterystats` spells this app.
	keys: Vec<String>,
	prev_cpu: Option<(u64, u64)>,
	prev_frames: Option<(u64, u64)>,
	prev_mah: Option<f64>,
	skipped: u32,
}

fn main() {
	let cli = Cli::parse();
	match run(&cli) {
		Ok(()) => {}
		// 0: run completed — 2: any error (device gone, package absent…).
		Err(e) => {
			eprintln!("perfpoll: {e:#}");
			std::process::exit(2);
		}
	}
}

fn run(cli: &Cli) -> Result<()> {
	let serial = pick_serial(cli.serial.as_deref())?;
	let dev = Adb::new(&serial);
	let interval = Duration::from_millis(cli.interval_ms.max(50));

	// Resolve uid up front: `batterystats` attributes drain to the uid
	// string, not always to the package name.
	let mut apps: Vec<(String, AppCtx)> = Vec::new();
	for pkg in &cli.packages {
		let dump = dev.shell(&["dumpsys", "package", pkg])?;
		let uid = parse_user_id(&dump).with_context(|| format!("no userId for {pkg}"))?;
		apps.push((
			pkg.clone(),
			AppCtx {
				// batterystats spells an app `u0a383` but a system uid as a
				// bare number (`UID 1000:`) — carry all three spellings.
				keys: vec![pkg.clone(), uid_string(uid), uid.to_string()],
				prev_cpu: None,
				prev_frames: None,
				prev_mah: None,
				skipped: 0,
			},
		));
	}

	let mut csv: Option<Box<dyn Write>> = if cli.json {
		None
	} else {
		let mut w: Box<dyn Write> = match &cli.out {
			Some(path) => Box::new(BufWriter::new(
				std::fs::File::create(path)
					.with_context(|| format!("create {}", path.display()))?,
			)),
			None => Box::new(BufWriter::new(io::stdout().lock())),
		};
		writeln!(w, "{CSV_HEADER}")?;
		w.flush()?;
		Some(w)
	};
	let mut samples: Vec<Sample> = Vec::new();

	let started = Instant::now();
	let deadline = (cli.duration_s > 0).then(|| started + Duration::from_secs(cli.duration_s));
	let mut next = started;
	let mut prev_tick = started;

	loop {
		if deadline.is_some_and(|d| Instant::now() >= d) {
			break;
		}
		let now = Instant::now();
		let elapsed = now.duration_since(prev_tick).as_secs_f64();
		prev_tick = now;
		let t_ms = now.duration_since(started).as_millis() as u64;

		// Shared per tick: total CPU jiffies and the global battery level.
		let proc_total = parse_proc_stat_total(&dev.shell(&["cat", "/proc/stat"])?);
		let battery = parse_battery_level(&dev.shell(&["dumpsys", "battery"])?);

		for (pkg, ctx) in &mut apps {
			let Some(pids) = dev.shell_opt(&["pidof", pkg]).filter(|s| !s.trim().is_empty()) else {
				// Not running this tick — keep the series honest, count it.
				ctx.skipped += 1;
				continue;
			};

			// CPU: sum utime+stime over the app's pids (each read may race
			// a dying process — that pid just drops out of the sum).
			let pid_ticks: u64 = parse_pids(&pids)
				.into_iter()
				.filter_map(|pid| {
					dev.shell_opt(&["cat", &format!("/proc/{pid}/stat")])
						.and_then(|s| parse_pid_ticks(&s))
				})
				.sum();
			let cpu = match (proc_total, ctx.prev_cpu) {
				(Some(total), Some(prev)) => cpu_pct(prev, (total, pid_ticks)),
				_ => None,
			};
			ctx.prev_cpu = proc_total.map(|total| (total, pid_ticks));

			let pss =
				dev.shell(&["dumpsys", "meminfo", pkg]).ok().and_then(|s| parse_meminfo_pss_kb(&s));
			let key_refs: Vec<&str> = ctx.keys.iter().map(String::as_str).collect();
			let mah_cum = dev
				.shell(&["dumpsys", "batterystats", pkg])
				.ok()
				.and_then(|s| parse_batterystats_mah(&s, &key_refs));
			let mah_delta = match (mah_cum, ctx.prev_mah) {
				(Some(curr), Some(prev)) if curr >= prev => Some(curr - prev),
				(Some(_), Some(_)) => Some(0.0), // counter reset: rebase
				_ => None,
			};
			ctx.prev_mah = mah_cum;

			let gfx = dev.shell(&["dumpsys", "gfxinfo", pkg]).ok().and_then(|s| parse_gfxinfo(&s));
			let (frames, janky, fps) = match (gfx, ctx.prev_frames) {
				(Some((f, j)), Some((pf, pj))) => {
					let df = delta(pf, f);
					let dj = delta(pj, j);
					(df, dj, df.map(|d| per_sec(d, elapsed)))
				}
				// First tick has no interval yet — absolute totals would
				// poison both the series and the summary.
				_ => (None, None, None),
			};
			ctx.prev_frames = gfx;

			let sample = Sample {
				t_ms,
				app: pkg.clone(),
				cpu_pct: cpu,
				pss_kb: pss,
				battery_pct: battery,
				mah_cum,
				mah_delta,
				frames,
				janky,
				fps,
			};
			if let Some(w) = csv.as_mut() {
				writeln!(w, "{}", sample.csv_row())?;
				w.flush()?;
			}
			samples.push(sample);
		}

		next += interval;
		let now = Instant::now();
		if next > now {
			std::thread::sleep(next - now);
		} else {
			next = now; // overrun (slow dumpsys): reschedule, never burst
		}
	}

	let elapsed_s = started.elapsed().as_secs_f64();
	if cli.json {
		write_json(cli, &serial, &samples, &apps, elapsed_s)?;
	} else if cli.out.is_some() {
		// stdout is free — keep it as the human-readable surface.
		print_summaries(&samples, &apps, io::stdout())?;
	} else {
		// CSV owns stdout; summaries must not pollute the stream.
		print_summaries(&samples, &apps, io::stderr())?;
	}
	Ok(())
}

/// Per-app summaries, in the order the packages were given.
fn summaries(samples: &[Sample], apps: &[(String, AppCtx)]) -> Vec<Summary> {
	apps.iter()
		.map(|(pkg, ctx)| {
			let own: Vec<Sample> = samples.iter().filter(|s| s.app == *pkg).cloned().collect();
			summarize(pkg, &own, ctx.skipped)
		})
		.collect()
}

/// One summary line per app. Fixed precision: a summary is for humans,
/// full floats stay in the JSON document.
fn print_summaries(
	samples: &[Sample],
	apps: &[(String, AppCtx)],
	mut out: impl Write,
) -> Result<()> {
	fn fixed(v: &Option<f64>, digits: usize, unit: &str) -> String {
		match v {
			Some(v) => format!("{v:.digits$}{unit}"),
			None => "-".to_string(),
		}
	}
	for s in summaries(samples, apps) {
		writeln!(
			out,
			"{}: {} samples, {} skipped | cpu avg {} | pss max {} kB | \
			 drained {} mAh | fps avg {} | jank {}% ({}/{})",
			s.app,
			s.samples,
			s.skipped,
			fixed(&s.avg_cpu_pct, 1, "%"),
			opt(&s.max_pss_kb, ""),
			fixed(&s.mah_drained, 2, ""),
			fixed(&s.avg_fps, 1, ""),
			fixed(&s.jank_pct, 1, ""),
			s.janky.map_or("?".into(), |v| v.to_string()),
			s.frames.map_or("?".into(), |v| v.to_string()),
		)?;
	}
	Ok(())
}

fn opt(v: &Option<impl std::fmt::Display>, unit: &str) -> String {
	match v {
		Some(v) => format!("{v}{unit}"),
		None => "-".to_string(),
	}
}

fn write_json(
	cli: &Cli,
	serial: &str,
	samples: &[Sample],
	apps: &[(String, AppCtx)],
	elapsed_s: f64,
) -> Result<()> {
	#[derive(serde::Serialize)]
	struct Document<'a> {
		serial: &'a str,
		interval_ms: u64,
		duration_s: u64,
		elapsed_s: f64,
		samples: &'a [Sample],
		summary: Vec<Summary>,
	}
	let doc = Document {
		serial,
		interval_ms: cli.interval_ms,
		duration_s: cli.duration_s,
		elapsed_s,
		samples,
		summary: summaries(samples, apps),
	};
	let text = serde_json::to_string_pretty(&doc)?;
	match &cli.out {
		Some(path) => std::fs::write(path, format!("{text}\n"))
			.with_context(|| format!("write {}", path.display()))?,
		None => println!("{text}"),
	}
	Ok(())
}
