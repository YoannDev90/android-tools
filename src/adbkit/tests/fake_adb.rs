//! Drives `adbkit`'s subprocess plumbing against a fake `adb`.
//!
//! The fake is a shell script whose stdout, stderr and exit code each check
//! canned, so `shell_opt` and `exec_out` are exercised end to end without
//! adb, a device, or a mocked layer — neither is needed in CI.
//!
//! One test, sequential **on purpose**. The fake adb is an executable this
//! test writes itself, and writing one while another thread of the same
//! process forks is an `ETXTBSY` race: the forked child inherits our
//! write-open fd (close-on-exec only drops it at the child's own exec), so
//! `close()` in this process leaves the inode's write count positive until
//! that child execs — and our own `execve` of the very same file fails with
//! "Text file busy" for those few microseconds. As parallel test threads
//! this failed roughly 1 run in 20; a single thread cannot, and a fork in
//! another *process* never inherits our fds (that is why these tests live
//! in their own integration binary, away from the `sh`-spawning unit
//! tests). So: no parallelism in this file, and no other test in the crate
//! writes an executable at runtime.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};

use adbkit::Adb;

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// An `Adb` whose binary is a shell script (`script` is its body).
///
/// The path is returned so the caller can clean up — this is the only
/// runtime write of an executable in the crate, hence the file-level rule
/// above.
fn fake_adb(script: &str) -> (Adb, std::path::PathBuf) {
	let path = std::env::temp_dir().join(format!(
		"adbkit-fake-adb-{}-{}",
		std::process::id(),
		NEXT.fetch_add(1, Ordering::Relaxed)
	));
	std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).expect("write fake adb");
	let mut perms = std::fs::metadata(&path).expect("stat").permissions();
	perms.set_mode(0o755);
	std::fs::set_permissions(&path, perms).expect("chmod");
	(Adb::with_program("SER123", path.to_string_lossy().into_owned()), path)
}

#[test]
fn shell_opt_and_exec_out_against_a_fake_adb() {
	// `command()` runs the right binary with `-s <serial>` first — asserted
	// on the child's own argv.
	let (adb, path) = fake_adb("for a in \"$@\"; do echo \"$a\"; done");
	assert_eq!(adb.serial(), "SER123");
	assert_eq!(
		adb.shell_opt(&["wm", "density"]).as_deref(),
		Some("-s\nSER123\nshell\nwm\ndensity\n")
	);
	let _ = std::fs::remove_file(path);

	// `shell_opt` returns the stdout whatever the exit code: failure is a
	// normal outcome (`pm path` on an absent package), so a non-zero exit
	// and a stderr message must still yield the stdout.
	let (adb, path) = fake_adb("echo ready");
	assert_eq!(adb.shell_opt(&["pidof", "com.foo"]).as_deref(), Some("ready\n"));
	let _ = std::fs::remove_file(path);

	let (adb, path) = fake_adb("echo partial; echo 'device offline' >&2; exit 1");
	assert_eq!(adb.shell_opt(&["pm", "path", "com.foo"]).as_deref(), Some("partial\n"));
	let _ = std::fs::remove_file(path);

	// `shell_opt` is None when adb cannot be spawned at all.
	let adb = Adb::with_program("SER123", "/definitely/not/here");
	assert_eq!(adb.shell_opt(&["true"]), None);

	// `exec_out` returns the stdout bytes verbatim: raw bytes, not text —
	// `screencap -p` hands back a PNG, and a lossy String round-trip would
	// corrupt it.
	let (adb, path) = fake_adb("printf '\\211PNG\\r\\n\\032\\n'");
	assert_eq!(adb.exec_out(&["screencap", "-p"]).unwrap(), b"\x89PNG\r\n\x1a\n");
	let _ = std::fs::remove_file(path);

	// A non-zero exit fails, naming the command and surfacing stderr.
	let (adb, path) = fake_adb("echo 'device offline' >&2; exit 1");
	let err = adb.exec_out(&["cat", "/sdcard/x.png"]).unwrap_err();
	let msg = err.to_string();
	assert!(msg.contains("adb exec-out cat /sdcard/x.png failed"), "{msg}");
	assert!(msg.contains("device offline"), "{msg}");
	let _ = std::fs::remove_file(path);

	// A spawn error keeps the context label (the io error is the source).
	let adb = Adb::with_program("SER123", "/definitely/not/here");
	let err = adb.exec_out(&["screencap", "-p"]).unwrap_err();
	assert!(err.to_string().contains("adb exec-out screencap -p"), "{err}");
}
