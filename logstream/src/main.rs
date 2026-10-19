mod adb;
mod buffer;
mod parse;
mod render;
mod tui;

use std::io::{IsTerminal, Write};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

	use anyhow::{Context, Result};
	use adbkit::pick_serial;
	use clap::Parser;

use buffer::{Banner, Buffer, Filter, PidMode};
use parse::Level;
use render::PlainRenderer;
use tui::App;

#[derive(Parser, Debug)]
#[command(
	name = "logstream",
	about = "Live Android log streamer — logcat, but actually nice",
	version
)]
struct Cli {
	/// App package name (release, then `.debug` is auto-tried).
	/// Omit to stream every process (no pid filtering).
	#[arg(short, long)]
	package: Option<String>,

	/// adb serial (auto-picked when a single device is connected).
	#[arg(short, long)]
	serial: Option<String>,

	/// Show every process (disable app-pid filtering).
	#[arg(long)]
	all: bool,

	/// Minimum level to display (v/d/i/w/e/f).
	#[arg(short, long, default_value = "v")]
	level: String,

	/// Keep only tags starting with this prefix (repeatable).
	#[arg(short = 't', long = "tag")]
	tags: Vec<String>,

	/// Drop tags starting with this prefix (repeatable).
	#[arg(long = "skip")]
	skips: Vec<String>,

	/// Initial search query (also filterable live with `/` in the TUI).
	#[arg(short = 'q', long, default_value = "")]
	search: String,

	/// Ring-buffer capacity (lines).
	#[arg(short, long, default_value_t = 10_000)]
	buffer: usize,

	/// Clear logcat buffer before streaming.
	#[arg(long)]
	clear: bool,

	/// Write JSON lines (one object per record) instead of the TUI.
	#[arg(long)]
	json: bool,

	/// Plain colored output (no TUI). Auto-selected when stdout is not a TTY.
	#[arg(long)]
	plain: bool,

	/// Mirror everything shown to this file: raw records plus `[logstream]`
	/// banner lines (works with any mode, including --json).
	#[arg(short, long)]
	out: Option<std::path::PathBuf>,
}

fn main() -> Result<()> {
	let cli = Cli::parse();

	let serial = pick_serial(cli.serial.as_deref())?;
	// Package is optional: without one, stream every process (no pid filter).
	let package = match cli.package.as_deref() {
		Some(pkg) => Some(adb::resolve_package(&serial, pkg)?),
		None => None,
	};
	let min_level = Level::from_str(&cli.level).context("--level must be one of v/d/i/w/e/f")?;

	let pid_mode = if cli.all || package.is_none() { PidMode::All } else { PidMode::App };

	let (tx, rx) = mpsc::channel();
	let logcat = adb::spawn_logcat(&serial, cli.clear, tx.clone())?;
	// The slot returned by the watcher *is* the shared slot (its thread holds
	// a clone and writes through it).
	let pid_slot: PidSlot = match &package {
		Some(pkg) => {
			let slot = adb::spawn_pid_watcher(serial.clone(), pkg.clone(), tx, Duration::from_millis(700));
			// Seed the current pid so the header is correct from frame 1.
			if let Some(pid) = adb::pid_of(&serial, pkg) {
				*log_slot_lock(&slot) = Some(pid);
			}
			slot
		}
		None => std::sync::Arc::new(std::sync::Mutex::new(None)),
	};

	let mut filter = Filter::new(pid_mode, package.clone().unwrap_or_default());
	filter.min_level = min_level;
	filter.allow = cli.tags.clone();
	filter.deny = cli.skips.clone();
	if !cli.search.is_empty() {
		filter.set_search(&cli.search);
	}
	refresh_pids(&mut filter, &pid_slot, pid_mode);

	let mut out_file = match &cli.out {
		Some(p) => Some(std::io::BufWriter::new(
			std::fs::File::create(p).with_context(|| format!("cannot create {}", p.display()))?,
		)),
		None => None,
	};

	let mode_plain = cli.plain || cli.json || !std::io::stdout().is_terminal();

	let package = package.unwrap_or_default();
	let result = if mode_plain {
		run_plain(&cli, filter, &rx, &pid_slot, &mut out_file, &package)
	} else {
		run_tui(filter, &rx, &pid_slot, &package, &mut out_file, cli.buffer)
	};

	logcat.kill();
	result
}

type PidSlot = std::sync::Arc<std::sync::Mutex<Option<u32>>>;

fn log_slot_lock(p: &PidSlot) -> std::sync::MutexGuard<'_, Option<u32>> {
	p.lock().unwrap()
}

/// Merge the current app pid into the filter's pid set (accumulating: pids of
/// dead processes are kept so crash history stays visible after a restart).
fn refresh_pids(filter: &mut Filter, slot: &PidSlot, mode: PidMode) {
	if mode == PidMode::All {
		filter.app_pids.clear();
		return;
	}
	for p in log_slot_lock(slot).iter().copied() {
		if !filter.app_pids.contains(&p) {
			filter.app_pids.push(p);
		}
	}
}

/// Blocking plain / JSONL mode.
fn run_plain(
	cli: &Cli,
	mut filter: Filter,
	rx: &Receiver<adb::Event>,
	pid_slot: &PidSlot,
	out_file: &mut Option<std::io::BufWriter<std::fs::File>>,
	package: &str,
) -> Result<()> {
	let color = !cli.json && std::io::stdout().is_terminal();
	let mut renderer = PlainRenderer::new(color);
	let mut raw = std::io::stdout().lock();
	let mut last_pid = *log_slot_lock(pid_slot);
	let mut exited = false;

	while let Ok(ev) = rx.recv() {
		if handle_event_plain(
			ev,
			&mut filter,
			&mut renderer,
			&mut raw,
			out_file,
			rx,
			pid_slot,
			package,
			&mut last_pid,
			&mut exited,
			cli.json,
		)? {
			break;
		}
	}
	Ok(())
}

/// Returns Ok(true) when the loop should stop.
#[allow(clippy::too_many_arguments)]
fn handle_event_plain(
	ev: adb::Event,
	filter: &mut Filter,
	renderer: &mut PlainRenderer,
	raw: &mut impl Write,
	out_file: &mut Option<std::io::BufWriter<std::fs::File>>,
	rx: &Receiver<adb::Event>,
	pid_slot: &PidSlot,
	package: &str,
	last_pid: &mut Option<u32>,
	exited: &mut bool,
	json: bool,
) -> Result<bool> {
	match ev {
		adb::Event::Record(rec) => {
			// Refresh pid set from the shared slot (cheap, accumulating).
			let mode = filter.pid_mode;
			refresh_pids(filter, pid_slot, mode);
			let is_crash = rec.tag == "AndroidRuntime" && rec.msg.contains("FATAL EXCEPTION");
			let at = rec.epoch_ms;
			if filter.keep(&rec) {
				let e = buffer_entry(rec);
				if json {
					let l = render::json_line(&e.record);
					writeln!(raw, "{l}")?;
				} else {
					renderer.write_to(raw, &e)?;
				}
				raw.flush()?;
				if let Some(f) = out_file.as_mut() {
					f.write_all(render::raw_line(&e.record).as_bytes())?;
					f.write_all(b"\n")?;
					f.flush()?;
				}
			}
			if is_crash {
				let e = entry_from_banner(Banner::Crash, at);
				if !json {
					renderer.write_to(raw, &e)?;
				}
				write_banner_note(out_file, &e.record.msg)?;
			}
		}
		adb::Event::PidChanged(pid) => {
			let old = last_pid.take();
			*last_pid = pid;
			if old != pid {
				let now = std::time::SystemTime::now()
					.duration_since(std::time::UNIX_EPOCH)
					.unwrap_or_default()
					.as_millis() as u64;
				let banner = match (old, pid) {
					(Some(o), Some(n)) => Some(Banner::Restart { old_pid: o, new_pid: n }),
					(Some(o), None) => Some(Banner::Died { pid: o }),
					(None, Some(n)) => Some(Banner::Started { pid: n }),
					(None, None) => None,
				};
				if let Some(b) = banner {
					let e = entry_from_banner(b, now);
					if !json {
						renderer.write_to(raw, &e)?;
						raw.flush()?;
					}
					write_banner_note(out_file, &e.record.msg)?;
				}
			}
			// If the app died, wait for a possible restart — no action needed,
			// the pid watcher keeps polling.
			let _ = package;
		}
		adb::Event::LogcatExited => {
			if !*exited {
				*exited = true;
				if !json {
					writeln!(raw, "\x1b[33m[logstream] adb logcat exited (device disconnected?)\x1b[0m")?;
					raw.flush()?;
				}
			}
			// Keep draining pid events, but records are gone: stop.
			while let Ok(ev) = rx.try_recv() {
				if matches!(ev, adb::Event::LogcatExited) {
					break;
				}
			}
			return Ok(true);
		}
		adb::Event::ParseError(_) => {}
	}
	Ok(false)
}

fn buffer_entry(rec: parse::Record) -> buffer::Entry {
	let last_ms = rec.epoch_ms;
	buffer::Entry {
		record: rec,
		count: 1,
		banner: None,
		last_ms,
	}
}

fn entry_from_banner(b: Banner, now: u64) -> buffer::Entry {
	buffer::Entry {
		record: parse::Record {
			epoch_ms: now,
			time: String::new(),
			pid: 0,
			tid: 0,
			level: Level::Warn,
			tag: "logstream".into(),
			msg: b.text(),
		},
		count: 1,
		banner: Some(b),
		last_ms: now,
	}
}

fn now_ms() -> u64 {
	std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis() as u64
}

/// `--out` mirror for banners: written in *every* mode (records are mirrored
/// as raw lines) so the file always reflects everything that was shown.
fn write_banner_note(out_file: &mut Option<std::io::BufWriter<std::fs::File>>, msg: &str) -> Result<()> {
	if let Some(f) = out_file.as_mut() {
		f.write_all(format!("[logstream] {msg}\n").as_bytes())?;
		f.flush()?;
	}
	Ok(())
}

/// TUI mode.
fn run_tui(
	mut filter: Filter,
	rx: &Receiver<adb::Event>,
	pid_slot: &PidSlot,
	package: &str,
	out_file: &mut Option<std::io::BufWriter<std::fs::File>>,
	buffer_cap: usize,
) -> Result<()> {
	use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
	use crossterm::execute;
	use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};
	use ratatui::backend::CrosstermBackend;
	use ratatui::Terminal;

	// Raw mode is what makes crossterm deliver KeyEvents at all: without it
	// the tty stays canonical and input is line-buffered until Enter (so no
	// key — PgUp, q, /… appears to do nothing).
	crossterm::terminal::enable_raw_mode()?;
	let mut stdout = std::io::stdout();
	// Alternate screen keeps the scrollback of the user's shell intact.
	execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
	let backend = CrosstermBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;
	terminal.clear()?;

	let mode = filter.pid_mode;
	refresh_pids(&mut filter, pid_slot, mode);
	let mut app = App::new(Buffer::new(buffer_cap), filter);
	app.not_running = log_slot_lock(pid_slot).is_none() && mode == PidMode::App;
	app.status = package.to_string();
	if app.not_running {
		let now = now_ms();
		app.push_banner(Banner::NotRunning, now);
	}

	let mut last_pid = *log_slot_lock(pid_slot);
	let result = event_loop(&mut app, &mut terminal, rx, pid_slot, out_file, &mut last_pid);

	// Teardown — always restore the terminal, whatever happened above.
	let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture);
	let _ = terminal.show_cursor();
	let _ = crossterm::terminal::disable_raw_mode();
	result
}

#[allow(clippy::too_many_arguments)]
fn event_loop<B: ratatui::backend::Backend>(
	app: &mut App,
	terminal: &mut ratatui::Terminal<B>,
	rx: &Receiver<adb::Event>,
	pid_slot: &PidSlot,
	out_file: &mut Option<std::io::BufWriter<std::fs::File>>,
	last_pid: &mut Option<u32>,
) -> Result<()> {
	loop {
		// Drain a bounded batch of stream events. An unbounded drain would
		// never yield under log spam, starving keyboard/mouse polling below.
		const MAX_BATCH: usize = 256;
		let mut batch = 0;
		while batch < MAX_BATCH {
			let ev = match rx.try_recv() {
				Ok(ev) => ev,
				Err(mpsc::TryRecvError::Empty) => break,
				Err(mpsc::TryRecvError::Disconnected) => return Ok(()),
			};
			batch += 1;
			match ev {
				adb::Event::Record(rec) => {
					let mode = app.filter.pid_mode;
					refresh_pids(&mut app.filter, pid_slot, mode);
					app.not_running = log_slot_lock(pid_slot).is_none() && mode == PidMode::App;
					let is_crash = rec.tag == "AndroidRuntime" && rec.msg.contains("FATAL EXCEPTION");
					if app.filter.keep(&rec) {
						if let Some(f) = out_file.as_mut() {
							f.write_all(render::raw_line(&rec).as_bytes())?;
							f.write_all(b"\n")?;
							f.flush()?;
						}
					}
					let at = rec.epoch_ms;
					app.push_record(rec);
					if is_crash {
						app.push_banner(Banner::Crash, at);
						write_banner_note(out_file, &Banner::Crash.text())?;
					}
				}
				adb::Event::PidChanged(pid) => {
					let old = last_pid.take();
					*last_pid = pid;
					let mode = app.filter.pid_mode;
					refresh_pids(&mut app.filter, pid_slot, mode);
					app.not_running = pid.is_none() && mode == PidMode::App;
					let now = std::time::SystemTime::now()
						.duration_since(std::time::UNIX_EPOCH)
						.unwrap_or_default()
						.as_millis() as u64;
					let banner = match (old, pid) {
						(Some(o), Some(n)) if o != n => Some(Banner::Restart { old_pid: o, new_pid: n }),
						(Some(o), None) => Some(Banner::Died { pid: o }),
						(None, Some(n)) => Some(Banner::Started { pid: n }),
						_ => None,
					};
					if let Some(b) = banner {
						write_banner_note(out_file, &b.text())?;
						app.push_banner(b, now);
					}
					app.status = match pid {
						Some(p) => format!("pid {p}"),
						None => "app not running".into(),
					};
				}
				adb::Event::LogcatExited => {
					app.status = "logcat exited (device disconnected?)".into();
				}
				adb::Event::ParseError(msg) => {
					app.status = format!("parse: {msg}");
				}
			}
		}

		app.tick();
		terminal
			.draw(|f| app.render(f))
			.map_err(|e| anyhow::anyhow!("terminal draw failed: {e}"))?;

		// Short poll: keep input responsive even while lines keep arriving.
		if tui::poll_events(app, Duration::from_millis(30))? {
			return Ok(());
		}
	}
}
