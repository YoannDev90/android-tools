//! `appctl` — the app-lifecycle actions of IntelliJ's adb-idea, as a
//! CLI that also works on every connected device at once.

mod cmds;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use adbkit::{list_serials, pick_serial, resolve_launcher, Adb};

use cmds::{argv, line, start_argv, Action};

#[derive(Parser, Debug)]
#[command(
	name = "appctl",
	about = "App lifecycle over adb — clear data/cache, restart, kill, uninstall, deep links",
	version
)]
struct Cli {
	/// adb serial; repeatable. Default: the single connected device.
	#[arg(short, long, global = true)]
	serial: Vec<String>,

	/// Run on every connected device.
	#[arg(long, global = true, conflicts_with = "serial")]
	all: bool,

	#[command(subcommand)]
	cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
	/// Clear the app's data and cache (`pm clear`).
	ClearData { package: String },
	/// Clear only the app's cache (`pm clear --cache-only`, Android 9+).
	ClearCache { package: String },
	/// Force-stop the app (not running is a success, not an error).
	Kill { package: String },
	/// Force-stop, then relaunch it on its launcher activity.
	Restart { package: String },
	/// Uninstall the package.
	Uninstall { package: String },
	/// Open a deep link (`am start -a VIEW -d <uri>`).
	Open { uri: String },
}

impl Cmd {
	fn action(&self) -> Action {
		match self {
			Cmd::ClearData { .. } => Action::ClearData,
			Cmd::ClearCache { .. } => Action::ClearCache,
			Cmd::Kill { .. } => Action::Kill,
			Cmd::Restart { .. } => Action::Restart,
			Cmd::Uninstall { .. } => Action::Uninstall,
			Cmd::Open { .. } => Action::Open,
		}
	}

	fn arg(&self) -> &str {
		match self {
			Cmd::ClearData { package }
			| Cmd::ClearCache { package }
			| Cmd::Kill { package }
			| Cmd::Restart { package }
			| Cmd::Uninstall { package } => package,
			Cmd::Open { uri } => uri,
		}
	}
}

fn main() -> Result<()> {
	let cli = Cli::parse();
	let serials = targets(&cli)?;
	let multi = serials.len() > 1;
	let mut failed = 0;
	for serial in &serials {
		let dev = Adb::new(serial);
		match one(&dev, &cli.cmd) {
			Ok(detail) => println!("{}", line(serial, multi, &detail)),
			Err(e) => {
				failed += 1;
				eprintln!("{}", line(serial, multi, &format!("{e:#}")));
			}
		}
	}
	if failed > 0 {
		bail!("{failed} of {} device(s) failed", serials.len());
	}
	Ok(())
}

/// Devices to act on: `--all`, the `-s` list, or the single one connected.
fn targets(cli: &Cli) -> Result<Vec<String>> {
	if cli.all {
		let serials = list_serials()?;
		if serials.is_empty() {
			bail!("no adb device connected");
		}
		return Ok(serials);
	}
	if !cli.serial.is_empty() {
		return Ok(cli.serial.clone());
	}
	Ok(vec![pick_serial(None)?])
}

/// One device, one action: the detail line that reports back.
fn one(dev: &Adb, cmd: &Cmd) -> Result<String> {
	let action = cmd.action();
	let arg = cmd.arg();
	if action == Action::Restart {
		// force-stop first — idempotent, a not-running app is a success.
		let stop_argv = argv(Action::Kill, arg)?;
		dev.shell(&refs(&stop_argv)).context("force-stop")?;
		// The launcher is resolved now, not hardcoded: it is whatever
		// this build declares, and it changes between versions.
		let component = resolve_launcher(dev, arg)?;
		let start_args = start_argv(&component);
		dev.shell(&refs(&start_args)).context("am start")?;
		return Ok(format!("restarted as {component}"));
	}
	let args = argv(action, arg)?;
	dev.shell(&refs(&args))?;
	Ok(match action {
		Action::ClearData => "data cleared".into(),
		Action::ClearCache => "cache cleared".into(),
		Action::Kill => "stopped".into(),
		Action::Uninstall => "uninstalled".into(),
		Action::Open => format!("opened {arg}"),
		Action::Restart => unreachable!(),
	})
}

/// `&[String]` → `&[&str]`, what the adbkit wrappers take.
fn refs(args: &[String]) -> Vec<&str> {
	args.iter().map(String::as_str).collect()
}
