//! Metric parsers — dumpsys/proc output in, numbers out, so the whole
//! sampling math tests without a device.

/// Jiffies on the aggregated `cpu` line of `/proc/stat` (all cores).
pub fn parse_proc_stat_total(out: &str) -> Option<u64> {
	let line = out.lines().find(|l| l.starts_with("cpu "))?;
	line.split_whitespace().skip(1).try_fold(0u64, |acc, f| f.parse::<u64>().ok().map(|v| acc + v))
}

/// `utime + stime` of `/proc/<pid>/stat`. The comm field can hold
/// spaces, so the split happens after the last `)`.
pub fn parse_pid_ticks(out: &str) -> Option<u64> {
	let rest = out.get(out.rfind(')')? + 1..)?;
	let mut fields = rest.split_whitespace();
	// state is field 3 … cmajflt is field 13 — then utime (14), stime (15).
	for _ in 0..11 {
		fields.next()?;
	}
	let utime: u64 = fields.next()?.parse().ok()?;
	let stime: u64 = fields.next()?.parse().ok()?;
	Some(utime + stime)
}

/// Sum of the per-process `TOTAL` rows of `dumpsys meminfo` (PSS, kB) —
/// one row per process of the app; `None` when no block was seen (app
/// not running). Summary sections after the blocks never match.
pub fn parse_meminfo_pss_kb(out: &str) -> Option<u64> {
	let mut in_block = false;
	let mut total = 0u64;
	let mut found = false;
	for line in out.lines() {
		if line.contains("** MEMINFO in pid") {
			in_block = true;
			continue;
		}
		if !in_block {
			continue;
		}
		if let Some(rest) = line.trim_start().strip_prefix("TOTAL ") {
			if let Some(v) = rest.split_whitespace().next().and_then(|n| n.parse::<u64>().ok()) {
				total += v;
				found = true;
				in_block = false;
			}
		}
	}
	found.then_some(total)
}

/// `level:` of `dumpsys battery`, 0..=100.
pub fn parse_battery_level(out: &str) -> Option<u32> {
	out.lines().find_map(|l| l.trim().strip_prefix("level:").and_then(|v| v.trim().parse().ok()))
}

/// `(total frames, janky frames)` of `dumpsys gfxinfo <pkg>`.
pub fn parse_gfxinfo(out: &str) -> Option<(u64, u64)> {
	let mut total = None;
	let mut janky = None;
	for line in out.lines() {
		if let Some(v) = line.strip_prefix("Total frames rendered:") {
			total = v.trim().parse().ok();
		} else if let Some(v) = line.strip_prefix("Janky frames:") {
			janky = v.trim().split([' ', '(']).next().and_then(|n| n.parse().ok());
		}
	}
	Some((total?, janky?))
}

/// Cumulative mAh of one app under `Estimated power use (mAh):`, matched
/// on the package name or its uid string. The section ends at the next
/// column-0 line.
pub fn parse_batterystats_mah(out: &str, keys: &[&str]) -> Option<f64> {
	let mut in_section = false;
	for line in out.lines() {
		let trimmed = line.trim();
		if trimmed.starts_with("Estimated power use") {
			in_section = true;
			continue;
		}
		if !in_section {
			continue;
		}
		if !line.starts_with(' ') && !line.is_empty() {
			break;
		}
		let Some(rest) = trimmed
			.strip_prefix("UID ")
			.or_else(|| trimmed.strip_prefix("Uid "))
			.or_else(|| trimmed.strip_prefix("uid "))
		else {
			continue;
		};
		let Some((token, value)) = rest.split_once(':') else { continue };
		if keys.contains(&token.trim()) {
			// Value carries a breakdown after the number: `12.5 fg: 6.90 (…)` —
			// first token only.
			if let Some(v) = value.split_whitespace().next().and_then(|n| n.parse::<f64>().ok()) {
				return Some(v);
			}
		}
	}
	None
}

/// How `batterystats` spells an uid: `u0a383` for apps (id ≥ 10000),
/// the bare number for system uids (`UID 1000:`).
pub fn uid_string(user_id: u32) -> String {
	let user = user_id / 100_000;
	let app = user_id % 100_000;
	if app < 10_000 {
		user_id.to_string()
	} else {
		format!("u{user}a{}", app - 10_000)
	}
}

/// The package's uid — accepts every spelling seen in the wild:
/// `userId=10123` (classic), `uid=10383` (Android 15 `dumpsys package`),
/// `uid:10383` (`pm list packages -U`).
pub fn parse_user_id(out: &str) -> Option<u32> {
	for token in out.split_whitespace() {
		for prefix in ["userId=", "uid=", "uid:"] {
			if let Some(v) = token.strip_prefix(prefix) {
				if let Ok(id) = v.parse::<u32>() {
					return Some(id);
				}
			}
		}
	}
	None
}

/// Pids from `pidof` (empty output / exit 1 = app not running).
pub fn parse_pids(out: &str) -> Vec<u32> {
	out.split_whitespace().filter_map(|t| t.parse().ok()).collect()
}

/// Process share of total CPU capacity, 0..=100 — the clamp absorbs
/// scheduler noise; `None` when time did not move or a counter reset
/// (process restart between ticks).
pub fn cpu_pct(prev: (u64, u64), curr: (u64, u64)) -> Option<f64> {
	let dt = curr.0.checked_sub(prev.0)?;
	let dp = curr.1.checked_sub(prev.1)?;
	if dt == 0 {
		return None;
	}
	Some((dp as f64 * 100.0 / dt as f64).clamp(0.0, 100.0))
}

/// Counters (frames, mAh) only grow: a backwards step means the app
/// restarted or the charge counter reset — rebase, never go negative.
pub fn delta(prev: u64, curr: u64) -> Option<u64> {
	curr.checked_sub(prev)
}

/// Rate per second over an actual elapsed time.
pub fn per_sec(delta: u64, elapsed_s: f64) -> f64 {
	if elapsed_s <= 0.0 {
		return 0.0;
	}
	delta as f64 / elapsed_s
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn proc_stat_sums_only_the_aggregate_line() {
		let out = "cpu  4478186 5261 1098442 1107 0 749 0 0 0\ncpu0 1200 50 1400 274000 300 0 200 0 0 0\nintr 99999\nctxt 987654\n";
		assert_eq!(parse_proc_stat_total(out), Some(5_583_745));
		assert_eq!(parse_proc_stat_total("no cpu line"), None);
	}

	#[test]
	fn pid_stat_splits_after_the_parenthesised_comm() {
		// comm contains a space — naive splitting would misalign the fields.
		let out = "12345 (my app) S 1 1 1 0 -1 4194560 100 0 0 0 300 400 0 0 20 0 5 0 0";
		assert_eq!(parse_pid_ticks(out), Some(700));
		assert_eq!(parse_pid_ticks("garbage"), None);
	}

	#[test]
	fn meminfo_sums_one_total_row_per_process_block() {
		let out = "\
Applications Memory Usage (in Kilobytes):
Uptime: 1000000 Realtime: 2000000

** MEMINFO in pid 1234 [com.example.app] **
                   Pss  Private  Private  SwapPss     Rss     Heap     Heap     Heap
                 Total    Dirty    Clean    Dirty Total     Size    Alloc     Free
                ------   ------   ------   ------   ------   ------   ------   ------
  Native Heap    12000    12000        0        0    12100    16384    12000     4000
        TOTAL    45678    30000     2000        0    60000        0        0        0

** MEMINFO in pid 1236 [com.example.app:remote] **
        TOTAL     9000     8000      500        0    11000        0        0        0

Total PSS by process:
    54678: com.example.app (pid 1234)
";
		assert_eq!(parse_meminfo_pss_kb(out), Some(54_678));
		assert_eq!(parse_meminfo_pss_kb("No process found for: com.x"), None);
	}

	#[test]
	fn battery_level_is_the_level_line() {
		let out = "Current Battery Service state:\n  status: 2\n  health: 2\n  level: 87\n  voltage: 4123\n";
		assert_eq!(parse_battery_level(out), Some(87));
		assert_eq!(parse_battery_level("nope"), None);
	}

	#[test]
	fn gfxinfo_reads_total_and_janky() {
		let out = "\
Stats since: 123456789ms
Total frames rendered: 1200
Janky frames: 14 (1.17%)
50th percentile: 6ms
Number Missed Vsync: 2
";
		assert_eq!(parse_gfxinfo(out), Some((1200, 14)));
		assert_eq!(parse_gfxinfo("no frames here"), None);
	}

	#[test]
	fn batterystats_matches_uid_or_package_inside_the_estimated_section() {
		let out = "\
Statistics since last charge:
  Estimated power use (mAh):
    Capacity: 4919, Computed drain: 196, actual drain: 344-394
    Global
      screen: 19.7 apps: 19.7 duration: 8m 46s 918ms
      cpu: 18.4 apps: 18.4
    UID u0a123: 12.5 fg: 6.90 ( cpu=4.15 (2m 33s 737ms) )
    Uid u0a456: 3.25

All partial wake locks:
  *ALR/SensorManager: 10s (12 times)
";
		assert_eq!(parse_batterystats_mah(out, &["com.example.app", "u0a123"]), Some(12.5));
		assert_eq!(parse_batterystats_mah(out, &["u0a456"]), Some(3.25));
		// System uids print as bare numbers: `UID 1000: 5.11 ( cpu=… )`.
		let sys = "  Estimated power use (mAh):\n    UID 1000: 5.11 ( cpu=4.86 )\n";
		assert_eq!(parse_batterystats_mah(sys, &["1000"]), Some(5.11));
		// Outside the section: the wake-lock block must not leak a number.
		assert_eq!(parse_batterystats_mah("Uid u0a123: 9.9", &["u0a123"]), None);
		assert_eq!(parse_batterystats_mah(out, &["com.absent"]), None);
	}

	#[test]
	fn uid_string_and_uid_lookup_cover_every_spelling() {
		assert_eq!(uid_string(10123), "u0a123");
		assert_eq!(uid_string(10456), "u0a456");
		// System uids are not "apps": bare number, as `UID 1000:` prints.
		assert_eq!(uid_string(1000), "1000");
		// Android 15 dumpsys package: `uid=10383`, never `userId=`.
		assert_eq!(parse_user_id("    uid=10383 gids=[] type=0"), Some(10383));
		assert_eq!(parse_user_id("userId=10123 minSdk=21"), Some(10123));
		assert_eq!(parse_user_id("package:com.x uid:10383"), Some(10383));
		// Must not bite on the installer token.
		assert_eq!(parse_user_id("    installerPackageUid=10103"), None);
		assert_eq!(parse_user_id("no ids"), None);
	}

	#[test]
	fn pids_come_from_pidof_output() {
		assert_eq!(parse_pids("1234 5678\n"), vec![1234, 5678]);
		assert!(parse_pids("\n").is_empty());
	}

	#[test]
	fn cpu_pct_uses_deltas_and_clamps() {
		assert_eq!(cpu_pct((1000, 5000), (1100, 5010)), Some(10.0));
		assert_eq!(cpu_pct((1000, 5000), (1000, 5010)), None); // no time
		assert_eq!(cpu_pct((1100, 5000), (1000, 5010)), None); // reset
		assert_eq!(cpu_pct((1000, 0), (1100, 2000)), Some(100.0)); // clamped
	}

	#[test]
	fn counters_never_go_negative_and_rates_use_real_elapsed() {
		assert_eq!(delta(100, 110), Some(10));
		assert_eq!(delta(110, 100), None); // restart / reset
		assert!((per_sec(30, 1.5) - 20.0).abs() < f64::EPSILON);
		assert_eq!(per_sec(30, 0.0), 0.0);
	}
}
