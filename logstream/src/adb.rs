use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Context, Result};

use adbkit::{Adb, ChildHandle};

use crate::parse::{parse_threadtime, Record};

/// Resolve the actual package name: try `pkg`, then `pkg.debug`.
pub fn resolve_package(serial: &str, pkg: &str) -> Result<String> {
	let adb = Adb::new(serial);
	for candidate in [pkg.to_string(), format!("{pkg}.debug")] {
		if let Some(out) = adb.shell_opt(&["pm", "path", &candidate]) {
			if out.contains("package:") {
				return Ok(candidate);
			}
		}
	}
	bail!("package {pkg} not installed on device")
}

/// Pid of `package` via `pidof` (fast, single process — app has no remote process).
pub fn pid_of(serial: &str, package: &str) -> Option<u32> {
	let adb = Adb::new(serial);
	let out = adb.shell(&["pidof", "-s", package]).ok()?;
	out.split_whitespace().next().and_then(|s| s.parse().ok())
}

/// Start `adb logcat` streaming into `tx`.
///
/// * `-T 1` — only the newest line of history, then follow.
/// * `-b main,system,crash` — crash buffer included (crashes live there).
/// * threadtime format parsed by [`parse_threadtime`]; continuation lines
///   (stack traces) are merged into the previous record here.
pub fn spawn_logcat(serial: &str, clear: bool, tx: Sender<Event>) -> Result<Arc<ChildHandle>> {
	if clear {
		let _ = Adb::new(serial).command().args(["logcat", "-c"]).output();
	}
	let mut child = Adb::new(serial)
		.command()
		.args(["logcat", "-v", "threadtime", "-b", "main,system,crash", "-T", "1"])
		.stdout(Stdio::piped())
		.stderr(Stdio::null())
		.spawn()
		.context("failed to spawn adb logcat")?;

	let stdout = child.stdout.take().context("no stdout")?;
	let handle = Arc::new(ChildHandle::new(child));

	let lc = Arc::clone(&handle);
	std::thread::spawn(move || {
		let mut last: Option<Record> = None;
		for line in BufReader::new(stdout).lines() {
			let Ok(line) = line else { break };
			match parse_threadtime(&line) {
				Ok(Some(rec)) => {
					if let Some(prev) = last.replace(rec) {
						if tx.send(Event::Record(prev)).is_err() {
							break;
						}
					}
				}
				Ok(None) => {
					// continuation (stacktrace) or banner line → append to prev
					if !line.starts_with("---------") {
						if let Some(prev) = last.as_mut() {
							prev.msg.push('\n');
							prev.msg.push_str(&line);
						}
					}
				}
				Err(e) => {
					let _ = tx.send(Event::ParseError(e.to_string()));
				}
			}
		}
		if let Some(prev) = last {
			let _ = tx.send(Event::Record(prev));
		}
		lc.kill();
		let _ = tx.send(Event::LogcatExited);
	});

	Ok(handle)
}

/// Poll the app pid every `every` ms; send [`Event::PidChanged`] on change.
pub fn spawn_pid_watcher(
	serial: String,
	package: String,
	tx: Sender<Event>,
	every: Duration,
) -> Arc<Mutex<Option<u32>>> {
	let current = Arc::new(Mutex::new(None::<u32>));
	let out_slot = Arc::clone(&current);
	std::thread::spawn(move || {
		loop {
			let pid = pid_of(&serial, &package);
			let mut guard = out_slot.lock().unwrap();
			if *guard != pid {
				*guard = pid;
				if tx.send(Event::PidChanged(pid)).is_err() {
					break;
				}
			}
			drop(guard);
			std::thread::sleep(every);
		}
	});
	current
}

pub enum Event {
	Record(Record),
	PidChanged(Option<u32>),
	LogcatExited,
	/// A line failed to parse (malformed logcat output); string is the reason.
	ParseError(String),
}
