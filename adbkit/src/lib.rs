//! Shared `adb` helpers for the android-tools workspace.
//!
//! Generic plumbing only: device picking, shell/exec-out wrappers with
//! timeouts. Domain logic (logcat streaming, UI hierarchy, screenshots)
//! stays in the tools that own it.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

/// Pick a serial: explicit flag, or the single connected device.
pub fn pick_serial(explicit: Option<&str>) -> Result<String> {
	if let Some(s) = explicit {
		return Ok(s.to_string());
	}
	let out = Command::new("adb")
		.args(["devices"])
		.output()
		.context("adb not found in PATH — install platform-tools")?;
	let mut devices = parse_devices(&String::from_utf8_lossy(&out.stdout));
	match devices.len() {
		0 => bail!("no adb device connected (or unauthorized)"),
		1 => Ok(devices.remove(0)),
		_ => bail!("multiple devices connected — pass --serial (found: {})", devices.join(", ")),
	}
}

/// Device ids in state `device` from `adb devices` stdout.
pub fn parse_devices(stdout: &str) -> Vec<String> {
	let mut devices = Vec::new();
	for line in stdout.lines().skip(1) {
		let mut it = line.split_whitespace();
		if let (Some(id), Some(state)) = (it.next(), it.next()) {
			if state == "device" {
				devices.push(id.to_string());
			}
		}
	}
	devices
}

/// A handle on one device: every command goes through `-s <serial>`.
pub struct Adb {
	serial: String,
}

impl Adb {
	pub fn new(serial: impl Into<String>) -> Self {
		Self { serial: serial.into() }
	}

	pub fn serial(&self) -> &str {
		&self.serial
	}

	pub fn command(&self) -> Command {
		let mut c = Command::new("adb");
		c.args(["-s", &self.serial]);
		c
	}

	/// `adb shell <args>`, stdout as String. Fails on non-zero exit.
	pub fn shell(&self, args: &[&str]) -> Result<String> {
		let out = self
			.command()
			.arg("shell")
			.args(args)
			.output()
			.with_context(|| format!("adb shell {}", args.join(" ")))?;
		if !out.status.success() {
			bail!(
				"adb shell {} failed: {}",
				args.join(" "),
				String::from_utf8_lossy(&out.stderr).trim()
			);
		}
		Ok(String::from_utf8_lossy(&out.stdout).into_owned())
	}

	/// `adb shell` with a hard timeout (uiautomator dump likes to hang).
	pub fn shell_timeout(&self, args: &[&str], timeout: Duration) -> Result<String> {
		let mut cmd = self.command();
		cmd.arg("shell").args(args);
		run_with_timeout(cmd, &format!("adb shell {}", args.join(" ")), timeout)
	}

	/// `adb shell` ignoring failures.
	pub fn shell_quiet(&self, args: &[&str]) {
		let _ = self.shell(args);
	}

	/// `adb shell <args>` ignoring non-zero exit (`(adb shell ... || true)`).
	/// For commands whose failure is a normal outcome (`pm path`, `pidof`).
	pub fn shell_opt(&self, args: &[&str]) -> Option<String> {
		self.command()
			.arg("shell")
			.args(args)
			.output()
			.ok()
			.map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
	}

	/// `adb exec-out <args>`, raw stdout bytes (screencap, cat).
	pub fn exec_out(&self, args: &[&str]) -> Result<Vec<u8>> {
		let out = self
			.command()
			.arg("exec-out")
			.args(args)
			.output()
			.with_context(|| format!("adb exec-out {}", args.join(" ")))?;
		if !out.status.success() {
			bail!(
				"adb exec-out {} failed: {}",
				args.join(" "),
				String::from_utf8_lossy(&out.stderr).trim()
			);
		}
		Ok(out.stdout)
	}
}

/// Launcher activity of a package (`cmd package resolve-activity --brief`).
pub fn resolve_launcher(dev: &Adb, package: &str) -> Result<String> {
	let out = dev
		.shell(&["cmd", "package", "resolve-activity", "--brief", package])
		.with_context(|| format!("resolve-activity {package}"))?;
	parse_component(&out).map(str::to_string).with_context(|| {
		format!("no launcher activity for {package:?} — pass the full component (pkg/.Activity)")
	})
}

/// Last `package/class` line of a `resolve-activity --brief` dump.
pub fn parse_component(out: &str) -> Option<&str> {
	out.lines().map(str::trim).rfind(|l| l.contains('/'))
}

/// Spawn `cmd`, kill it after `timeout`, return its stdout.
///
/// The pipes are drained on threads: a child that writes more than the pipe
/// buffer (~64 KiB) would block on write, never exit, and the try_wait loop
/// below would spin until the timeout killed it.
///
/// `label` is what appears in error messages (`adb shell …`).
fn run_with_timeout(mut cmd: Command, label: &str, timeout: Duration) -> Result<String> {
	let mut child = cmd
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::piped())
		.spawn()
		.with_context(|| label.to_string())?;
	let stdout = read_pipe(child.stdout.take());
	let stderr = read_pipe(child.stderr.take());
	let start = Instant::now();
	let status = loop {
		if let Some(status) = child.try_wait()? {
			break status;
		}
		if start.elapsed() > timeout {
			let _ = child.kill();
			let _ = child.wait();
			bail!("{label} timed out after {timeout:?}");
		}
		std::thread::sleep(Duration::from_millis(50));
	};
	// The child exited (or was killed): the pipes are closed and the
	// reader threads have (or soon will) hit EOF — joining makes the
	// captured output complete.
	let out = stdout.join().unwrap_or_default();
	if !status.success() {
		let err = stderr.join().unwrap_or_default();
		bail!("{label} failed: {}", err.trim());
	}
	Ok(out)
}

/// Read a piped stream to EOF on its own thread (see [`run_with_timeout`]).
fn read_pipe<T: Read + Send + 'static>(pipe: Option<T>) -> std::thread::JoinHandle<String> {
	std::thread::spawn(move || {
		let mut buf = Vec::new();
		if let Some(mut p) = pipe {
			let _ = p.read_to_end(&mut buf);
		}
		String::from_utf8_lossy(&buf).into_owned()
	})
}

/// Shared handle on a spawned child so the main thread can kill it.
pub struct ChildHandle {
	child: std::sync::Mutex<Child>,
}

impl ChildHandle {
	pub fn new(child: Child) -> Self {
		Self { child: std::sync::Mutex::new(child) }
	}

	pub fn kill(&self) {
		if let Ok(mut c) = self.child.lock() {
			let _ = c.kill();
			let _ = c.wait();
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn parse_devices_keeps_only_ready() {
		let out = "List of devices attached\n\
		           emulator-5554\tdevice\n\
		           192.168.1.4:5555\tdevice\n\
		           0a1b2c3d\tunauthorized\n\
		           \n";
		assert_eq!(
			parse_devices(out),
			vec!["emulator-5554".to_string(), "192.168.1.4:5555".to_string()]
		);
	}

	#[test]
	fn parse_devices_empty() {
		assert!(parse_devices("List of devices attached\n").is_empty());
	}

	#[test]
	fn pick_serial_fails_gracefully_without_adb() {
		// Can't assert success (depends on env); just ensure it doesn't panic on parse.
		if Command::new("adb").arg("--version").output().is_err() {
			assert!(pick_serial(None).is_err());
		}
	}

	#[test]
	fn parse_component_takes_last_slash_line() {
		let out = "priority=0 preferredOrder=0 match=0x108000 specificIndex=-1 isDefault=true\ncom.foo/.MainActivity\n";
		assert_eq!(parse_component(out), Some("com.foo/.MainActivity"));
	}

	#[test]
	fn parse_component_skips_lines_without_component() {
		let out = "No activity found\ncom.foo/com.foo.Other\n";
		assert_eq!(parse_component(out), Some("com.foo/com.foo.Other"));
	}

	#[test]
	fn parse_component_none_without_activity() {
		assert_eq!(parse_component("No activity found\n"), None);
		assert_eq!(parse_component(""), None);
	}

	// `sh`-driven tests of the timeout core: they exercise the exact code path
	// `adb shell` uses, without needing a device (or adb) installed.
	#[cfg(unix)]
	mod timeout {
		use super::*;

		fn sh(script: &str) -> Command {
			let mut c = Command::new("sh");
			c.arg("-c").arg(script);
			c
		}

		#[test]
		fn large_output_does_not_deadlock() {
			// >64 KiB (the pipe buffer): without thread draining, this child
			// blocks on write, never exits, and hits the timeout instead.
			let out = run_with_timeout(sh("seq 1 20000"), "seq", Duration::from_secs(10))
				.expect("must complete, not time out");
			assert!(out.len() > 64 * 1024, "only {} bytes", out.len());
			assert!(out.starts_with("1\n"), "{}", &out[..20.min(out.len())]);
			assert!(out.trim_end().ends_with("20000"));
		}

		#[test]
		fn slow_child_is_killed_at_timeout() {
			let start = Instant::now();
			let err = run_with_timeout(sh("sleep 30"), "sleep", Duration::from_millis(300))
				.expect_err("sleep must be killed");
			assert!(err.to_string().contains("timed out"), "{err}");
			// The kill is prompt: well under the child's own 30s.
			assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
		}

		#[test]
		fn failure_surfaces_stderr_and_exit_status() {
			let err = run_with_timeout(sh("echo boom >&2; exit 3"), "boom", Duration::from_secs(5))
				.expect_err("non-zero exit must fail");
			let msg = err.to_string();
			assert!(msg.contains("failed"), "{msg}");
			assert!(msg.contains("boom"), "{msg}");
		}

		#[test]
		fn missing_program_reports_spawn_error() {
			let mut c = Command::new("/definitely/not/here");
			c.arg("x");
			let err = run_with_timeout(c, "spawn", Duration::from_secs(5)).expect_err("spawn");
			// Not found at spawn time: the context label, not the exit path.
			assert!(err.to_string().contains("spawn"), "{err}");
		}
	}
}
