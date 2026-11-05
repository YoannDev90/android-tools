//! The lint rules. Pure: they read the hierarchy model from `adbkit` and
//! emit [`Finding`]s — no device, no files.
//!
//! The node set is `adbkit::hierarchy::parse_hierarchy`: what `uitool`
//! acts on — nodes carrying a label or a resource-id. That filter is
//! deliberate here too: a clickable row whose label sits on its children
//! (the standard Android pattern) must not be flagged, while an icon-only
//! clickable view (id, nothing rendered) is exactly what rule 3 hunts.

use serde::Serialize;

use adbkit::hierarchy::{Bounds, Node};

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

/// Rule 2 (error): two tap targets stacked on each other — the
/// intersection covers more than 20% of the smaller one, so a tap cannot
/// reliably tell them apart.
fn overlap(nodes: &[Node], out: &mut Vec<Finding>) {
	let targets: Vec<(&Node, Bounds)> = nodes
		.iter()
		.filter(|n| n.self_clickable)
		.filter_map(|n| n.bounds.map(|b| (n, b)))
		.collect();
	for (i, (a, ba)) in targets.iter().enumerate() {
		for (b, bb) in &targets[i + 1..] {
			let (x1, y1) = (ba.x1.max(bb.x1), ba.y1.max(bb.y1));
			let (x2, y2) = (ba.x2.min(bb.x2), ba.y2.min(bb.y2));
			if x2 <= x1 || y2 <= y1 {
				continue; // disjoint on one axis
			}
			let inter = (x2 - x1) as i64 * (y2 - y1) as i64;
			let smaller = ((ba.x2 - ba.x1) as i64 * (ba.y2 - ba.y1) as i64)
				.min((bb.x2 - bb.x1) as i64 * (bb.y2 - bb.y1) as i64);
			if smaller > 0 && inter * 100 > smaller * 20 {
				out.push(Finding {
					severity: Severity::Error,
					rule: "overlap",
					message: format!(
						"targets {:?} and {:?} overlap by {:.0}% of the smaller",
						a.label(),
						b.label(),
						inter as f64 * 100.0 / smaller as f64
					),
					bounds: Some([ba.x1, ba.y1, ba.x2, ba.y2]),
				});
			}
		}
	}
}

/// Rule 3 (error): a tap target nothing identifies — clickable, but
/// neither `text` nor `content-desc`. An id keeps the node in the model
/// but is not rendered, so the user (and any script driven by labels) has
/// nothing to aim at.
fn unlabeled(nodes: &[Node], out: &mut Vec<Finding>) {
	for n in nodes {
		if n.self_clickable && !n.has_visible_label() {
			out.push(Finding {
				severity: Severity::Error,
				rule: "unlabeled",
				message: format!("target {:?} is clickable but has no label", n.label()),
				bounds: n.bounds.map(|b| [b.x1, b.y1, b.x2, b.y2]),
			});
		}
	}
}

/// Rule 4 (warning): the label is displayed on a node you cannot tap
/// while an identical clickable one exists — the ambiguity that makes
/// `tap "Wi-Fi"` hit the toolbar title instead of the row. The flag sits
/// on the non-clickable occurrence: that is the one a user aims at and
/// that gets matched first.
fn duplicate_label(nodes: &[Node], out: &mut Vec<Finding>) {
	for n in nodes {
		if n.self_clickable || displayed(n).is_none() {
			continue;
		}
		let shown = displayed(n).expect("checked above");
		let twin = nodes.iter().filter(|o| o.self_clickable).any(|o| displayed(o) == Some(shown));
		if twin {
			out.push(Finding {
				severity: Severity::Warning,
				rule: "duplicate-label",
				message: format!(
					"label {:?} is not tappable here, but an identical clickable target exists",
					shown
				),
				bounds: n.bounds.map(|b| [b.x1, b.y1, b.x2, b.y2]),
			});
		}
	}
}

/// What a human reads on this node: `text`, else `content-desc` — and
/// deliberately nothing when both are empty (an id is not displayed).
fn displayed(n: &Node) -> Option<&str> {
	if !n.text.is_empty() {
		Some(&n.text)
	} else if !n.desc.is_empty() {
		Some(&n.desc)
	} else {
		None
	}
}

/// Lint the whole hierarchy against `opts`; findings come out in rule
/// order, then node order — a stable order CI diffs can rely on.
pub fn lint(nodes: &[Node], opts: &Opts) -> Vec<Finding> {
	let mut out = Vec::new();
	touch_size(nodes, opts, &mut out);
	overlap(nodes, &mut out);
	unlabeled(nodes, &mut out);
	duplicate_label(nodes, &mut out);
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

	/// A row and an icon button stacked on it (the classic ambiguous tap
	/// target), plus two targets that merely touch.
	const OVERLAP: &str = r#"<hierarchy rotation="0">
		<node index="0" text="Row" resource-id="com.foo:id/row" class="android.widget.LinearLayout" package="com.foo" clickable="true" bounds="[0,0][1080,160]"/>
		<node index="1" text="Chevron" resource-id="com.foo:id/chevron" class="android.widget.ImageView" package="com.foo" clickable="true" bounds="[1000,0][1080,160]"/>
		<node index="2" text="Next" resource-id="com.foo:id/next" class="android.widget.Button" package="com.foo" clickable="true" bounds="[0,160][1080,320]"/>
		<node index="3" text="Edge" resource-id="com.foo:id/edge" class="android.widget.Button" package="com.foo" clickable="true" bounds="[0,320][540,480]"/>
	</hierarchy>"#;

	#[test]
	fn stacked_targets_are_an_error_disjoint_ones_are_not() {
		let nodes = parse_hierarchy(OVERLAP);
		let opts = Opts { min_touch: 48, density: 160 };
		let findings = lint(&nodes, &opts);
		// Row vs Chevron overlap: the chevron (12800 px) is entirely inside
		// the row (172800 px) = 100% of the smaller. Row vs Next and
		// Next vs Edge merely touch — zero area.
		assert_eq!(findings.len(), 1, "{findings:?}");
		let f = &findings[0];
		assert_eq!(f.severity, Severity::Error);
		assert_eq!(f.rule, "overlap");
		assert!(f.message.contains("\"Row\" and \"Chevron\""), "{}", f.message);
		assert!(f.message.contains("100%"), "{}", f.message);
	}

	#[test]
	fn overlap_below_twenty_percent_is_tolerated() {
		// A 1080x160 row with a 1080x150 target inside: 93.75% — flagged.
		let xml = r#"<hierarchy rotation="0">
			<node text="A" clickable="true" bounds="[0,0][1080,160]"/>
			<node text="B" clickable="true" bounds="[0,0][1080,150]"/>
		</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		assert_eq!(lint(&nodes, &Opts { min_touch: 48, density: 160 }).len(), 1);
		// Push B out until only 10% of it is covered: tolerated.
		let xml = r#"<hierarchy rotation="0">
			<node text="A" clickable="true" bounds="[0,0][1080,160]"/>
			<node text="B" clickable="true" bounds="[0,145][1080,305]"/>
		</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		assert!(lint(&nodes, &Opts { min_touch: 48, density: 160 }).is_empty());
	}

	#[test]
	fn unlabeled_clickable_is_an_error() {
		// The icon-only view: kept in the model by its resource-id, nothing
		// rendered. A labelled target and a decorative icon are not flagged.
		let xml = r#"<hierarchy rotation="0">
			<node text="" content-desc="" resource-id="com.foo:id/mic" class="android.widget.ImageButton" clickable="true" bounds="[900,0][1080,160]"/>
			<node text="Search" content-desc="" resource-id="com.foo:id/search" class="android.widget.Button" clickable="true" bounds="[0,0][100,160]"/>
			<node text="" content-desc="" resource-id="com.foo:id/deco" class="android.widget.ImageView" clickable="false" bounds="[200,0][300,160]"/>
		</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		let findings = lint(&nodes, &Opts { min_touch: 48, density: 160 });
		assert_eq!(findings.len(), 1, "{findings:?}");
		let f = &findings[0];
		assert_eq!(f.severity, Severity::Error);
		assert_eq!(f.rule, "unlabeled");
		assert!(f.message.contains("com.foo:id/mic"), "{}", f.message);
		assert_eq!(f.bounds, Some([900, 0, 1080, 160]));
	}

	#[test]
	fn content_desc_counts_as_a_label() {
		let xml = r#"<hierarchy rotation="0">
			<node text="" content-desc="Close" resource-id="com.foo:id/close" clickable="true" bounds="[0,0][160,160]"/>
		</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		assert!(lint(&nodes, &Opts { min_touch: 48, density: 160 }).is_empty());
	}

	#[test]
	fn label_on_a_non_clickable_twin_is_a_warning() {
		// The uitool "Wi-Fi" case: toolbar title (not tappable) and row
		// (tappable) show the same label.
		let xml = r#"<hierarchy rotation="0">
			<node text="Wi-Fi" content-desc="" resource-id="com.foo:id/title" class="android.widget.TextView" clickable="false" bounds="[54,219][219,303]"/>
			<node text="Wi-Fi" content-desc="" resource-id="com.foo:id/row" class="android.widget.LinearLayout" clickable="true" bounds="[25,333][1055,450]"/>
			<node text="On" content-desc="" resource-id="com.foo:id/status" class="android.widget.TextView" clickable="false" bounds="[955,866][983,903]"/>
		</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		let findings = lint(&nodes, &Opts { min_touch: 48, density: 160 });
		assert_eq!(findings.len(), 1, "{findings:?}");
		let f = &findings[0];
		assert_eq!(f.severity, Severity::Warning);
		assert_eq!(f.rule, "duplicate-label");
		assert!(f.message.contains("\"Wi-Fi\""), "{}", f.message);
		// Flagged on the non-tappable occurrence, not the row.
		assert_eq!(f.bounds, Some([54, 219, 219, 303]));
	}

	#[test]
	fn a_unique_label_is_not_a_duplicate() {
		let xml = r#"<hierarchy rotation="0">
			<node text="Send" resource-id="com.foo:id/send" class="android.widget.TextView" clickable="false" bounds="[0,0][100,160]"/>
		</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		assert!(lint(&nodes, &Opts { min_touch: 48, density: 160 }).is_empty());
	}

	/// Acceptance fixture: all four rules fire on one screen — an
	/// undersized target, two stacked ones, an icon-only target, and a
	/// label sitting on a non-tappable title.
	const ALL_FOUR: &str = r#"<hierarchy rotation="0">
		<node index="0" text="Wi-Fi" content-desc="" resource-id="com.foo:id/actionbar_title" class="android.widget.TextView" clickable="false" bounds="[54,80][219,164]"/>
		<node index="1" text="Wi-Fi" content-desc="" resource-id="com.foo:id/row" class="android.widget.LinearLayout" clickable="true" bounds="[25,200][1055,340]"/>
		<node index="2" text="" content-desc="More options" resource-id="com.foo:id/row_chevron" class="android.widget.ImageView" clickable="true" bounds="[915,200][1055,340]"/>
		<node index="3" text="Switch" content-desc="" resource-id="com.foo:id/switch" class="android.widget.Switch" clickable="true" bounds="[950,360][1040,420]"/>
		<node index="4" text="" content-desc="" resource-id="com.foo:id/mic" class="android.widget.ImageButton" clickable="true" bounds="[900,440][1040,520]"/>
	</hierarchy>"#;

	#[test]
	fn all_four_rules_fire_on_one_screen() {
		let nodes = parse_hierarchy(ALL_FOUR);
		let findings = lint(&nodes, &Opts { min_touch: 48, density: 440 });
		let by_rule = |r: &str| findings.iter().filter(|f| f.rule == r).count();
		// Switch (90x60 px = 32.7x21.8 dp) and mic are under 48 dp...
		assert_eq!(by_rule("touch-size"), 2, "{findings:?}");
		// …row and chevron are stacked (chevron fully inside)…
		assert_eq!(by_rule("overlap"), 1, "{findings:?}");
		// …the mic target renders nothing…
		assert_eq!(by_rule("unlabeled"), 1, "{findings:?}");
		// …and "Wi-Fi" shows on the untappable title.
		assert_eq!(by_rule("duplicate-label"), 1, "{findings:?}");
		assert_eq!(findings.iter().filter(|f| f.severity == Severity::Error).count(), 2);
		assert_eq!(findings.iter().filter(|f| f.severity == Severity::Warning).count(), 3);
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
