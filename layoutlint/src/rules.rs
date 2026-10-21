//! The lint rules. Pure: they read the hierarchy model from `adbkit` and
//! emit [`Finding`]s — no device, no files.
//!
//! The node set is `adbkit::hierarchy::parse_hierarchy`: what `uitool`
//! acts on — nodes carrying a label or a resource-id. That filter is
//! deliberate here too: a clickable row whose label sits on its children
//! (the standard Android pattern) must not be flagged, while an icon-only
//! clickable view (id, nothing rendered) is exactly what rule 3 hunts.

use serde::Serialize;

use adbkit::hierarchy::Node;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
	Error,
	Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
	pub severity: Severity,
	/// Stable rule id: `touch-size`, `overlap`, `unlabeled`,
	/// `duplicate-label`.
	pub rule: &'static str,
	pub message: String,
	/// The offending node's bounds `[x1, y1, x2, y2]`.
	pub bounds: Option<[i32; 4]>,
}

/// Tuning knobs shared by the rules.
pub struct Opts {
	/// Minimum touch target in dp (Android's accessibility guidance: 48).
	pub min_touch: u32,
	/// Screen density in dpi for px→dp (device's in --device mode).
	pub density: u32,
}

impl Opts {
	/// `min_touch` dp expressed in px: the integer form of
	/// `min_touch * density / 160`, rounded up so a target exactly at the
	/// guidance is never flagged by a truncation.
	fn min_touch_px(&self) -> i64 {
		(self.min_touch as i64 * self.density as i64 + 159) / 160
	}
}

/// Rule 1 (warning): a self-clickable node whose bounds are smaller than
/// the minimum touch target in either dimension.
fn touch_size(nodes: &[Node], opts: &Opts, out: &mut Vec<Finding>) {
	let min_px = opts.min_touch_px();
	for n in nodes {
		if !n.self_clickable {
			continue;
		}
		let Some(b) = n.bounds else { continue };
		let (w, h) = (b.x2 - b.x1, b.y2 - b.y1);
		if (w as i64) < min_px || (h as i64) < min_px {
			out.push(Finding {
				severity: Severity::Warning,
				rule: "touch-size",
				message: format!(
					"target {:?} is {:.1}x{:.1} dp (min {})",
					n.label(),
					dp(w, opts.density),
					dp(h, opts.density),
					opts.min_touch
				),
				bounds: Some([b.x1, b.y1, b.x2, b.y2]),
			});
		}
	}
}

/// px → dp for display, one decimal.
fn dp(px: i32, density: u32) -> f64 {
	px as f64 * 160.0 / density as f64
}

/// Lint the whole hierarchy against `opts`; findings come out in rule
/// order, then node order — a stable order CI diffs can rely on.
pub fn lint(nodes: &[Node], opts: &Opts) -> Vec<Finding> {
	let mut out = Vec::new();
	touch_size(nodes, opts, &mut out);
	out
}

pub fn render_text(nodes: &[Node], opts: &Opts, findings: &[Finding]) -> String {
	if findings.is_empty() {
		return format!("{} nodes linted at {} dp — no finding", nodes.len(), opts.min_touch);
	}
	let mut s = format!("{} nodes linted at {} dp\n", nodes.len(), opts.min_touch);
	for f in findings {
		s.push_str(&format!(
			"{:<8} {:<16} {}  {}\n",
			format!("{:?}", f.severity).to_lowercase(),
			f.rule,
			f.message,
			f.bounds.map_or_else(|| "-".to_string(), fmt_bounds)
		));
	}
	let errors = findings.iter().filter(|f| f.severity == Severity::Error).count();
	let warnings = findings.len() - errors;
	s.push_str(&format!(
		"{} finding{} ({} error{}, {} warning{})\n",
		findings.len(),
		if findings.len() == 1 { "" } else { "s" },
		errors,
		if errors == 1 { "" } else { "s" },
		warnings,
		if warnings == 1 { "" } else { "s" }
	));
	s
}

fn fmt_bounds([x1, y1, x2, y2]: [i32; 4]) -> String {
	format!("[{x1},{y1}][{x2},{y2}]")
}

#[derive(Serialize)]
struct Report<'a> {
	nodes: usize,
	density: u32,
	min_touch: u32,
	findings: &'a [Finding],
	errors: usize,
	warnings: usize,
	failed: bool,
}

pub fn render_json(
	nodes: &[Node],
	opts: &Opts,
	findings: &[Finding],
	failed: bool,
) -> Result<String, serde_json::Error> {
	let errors = findings.iter().filter(|f| f.severity == Severity::Error).count();
	serde_json::to_string_pretty(&Report {
		nodes: nodes.len(),
		density: opts.density,
		min_touch: opts.min_touch,
		findings,
		errors,
		warnings: findings.len() - errors,
		failed,
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	use adbkit::hierarchy::parse_hierarchy;

	/// A screen with one target just under the 48 dp guidance (at 440 dpi),
	/// one comfortably over, and one tiny node that is not tappable — only
	/// the first may be flagged.
	const SMALL_TARGET: &str = r#"<hierarchy rotation="0">
		<node index="0" text="Send" resource-id="com.foo:id/send" class="android.widget.Button" package="com.foo" content-desc="" checkable="false" checked="false" clickable="true" enabled="true" focusable="true" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[800,1800][900,1892]"/>
		<node index="1" text="Cancel" resource-id="com.foo:id/cancel" class="android.widget.Button" package="com.foo" content-desc="" checkable="false" checked="false" clickable="true" enabled="true" focusable="true" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[400,1800][700,1960]"/>
		<node index="2" text="" resource-id="com.foo:id/divider" class="android.view.View" package="com.foo" content-desc="" checkable="false" checked="false" clickable="false" enabled="true" focusable="false" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[0,1790][1080,1792]"/>
	</hierarchy>"#;

	#[test]
	fn undersized_clickable_is_a_warning() {
		let nodes = parse_hierarchy(SMALL_TARGET);
		let opts = Opts { min_touch: 48, density: 440 };
		let findings = lint(&nodes, &opts);
		assert_eq!(findings.len(), 1, "{findings:?}");
		let f = &findings[0];
		assert_eq!(f.severity, Severity::Warning);
		assert_eq!(f.rule, "touch-size");
		// 100x92 px at 440 dpi = 36.4x33.5 dp.
		assert!(f.message.contains("36.4x33.5 dp (min 48)"), "{}", f.message);
		assert_eq!(f.bounds, Some([800, 1800, 900, 1892]));
	}

	#[test]
	fn non_clickable_tiny_node_is_never_flagged() {
		let nodes = parse_hierarchy(SMALL_TARGET);
		let opts = Opts { min_touch: 48, density: 440 };
		// The divider is 2 px tall but not clickable: not a touch target.
		assert!(!lint(&nodes, &opts).iter().any(|f| f.bounds == Some([0, 1790, 1080, 1792])));
	}

	#[test]
	fn a_target_exactly_at_the_guidance_passes() {
		// 48 dp at 440 dpi = 132 px exactly: not flagged.
		let xml = r#"<hierarchy rotation="0">
			<node text="Just right" clickable="true" bounds="[0,0][132,132]"/>
		</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		assert!(lint(&nodes, &Opts { min_touch: 48, density: 440 }).is_empty());
		// One pixel less is flagged.
		let xml = xml.replace("132,132", "131,131");
		let nodes = parse_hierarchy(&xml);
		assert_eq!(lint(&nodes, &Opts { min_touch: 48, density: 440 }).len(), 1);
	}

	#[test]
	fn text_report_lists_findings_and_counts() {
		let nodes = parse_hierarchy(SMALL_TARGET);
		let opts = Opts { min_touch: 48, density: 440 };
		let findings = lint(&nodes, &opts);
		let t = render_text(&nodes, &opts, &findings);
		assert!(t.contains("touch-size"), "{t}");
		assert!(t.contains("[800,1800][900,1892]"), "{t}");
		assert!(t.contains("1 warning"), "{t}");
	}

	#[test]
	fn json_report_carries_the_shape() {
		let nodes = parse_hierarchy(SMALL_TARGET);
		let opts = Opts { min_touch: 48, density: 440 };
		let findings = lint(&nodes, &opts);
		let j = render_json(&nodes, &opts, &findings, true).unwrap();
		let v: serde_json::Value = serde_json::from_str(&j).unwrap();
		assert_eq!(v["density"], 440);
		assert_eq!(v["min_touch"], 48);
		assert_eq!(v["warnings"], 1);
		assert_eq!(v["errors"], 0);
		assert_eq!(v["failed"], true);
		assert_eq!(v["findings"][0]["rule"], "touch-size");
	}
}
