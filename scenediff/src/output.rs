//! Report rendering: human text and JSON.
//!
//! One [`Report`] feeds both, so the two renderings cannot drift apart.

use std::path::Path;

use anyhow::Result;
use serde::Serialize;

use crate::diff::Diff;

#[derive(Debug, PartialEq, Serialize)]
pub struct Report {
	pub a: String,
	pub b: String,
	pub width: u32,
	pub height: u32,
	pub compared: usize,
	pub differing: usize,
	pub percent: f64,
	/// `[x1, y1, x2, y2]`, inclusive, in compared-image pixels.
	pub bbox: Option<[u32; 4]>,
	/// Overlay image written, if any.
	pub out: Option<String>,
	pub threshold: f64,
	/// True when the diff exceeded the threshold (exit 1).
	pub failed: bool,
}

impl Report {
	pub fn new(a: &str, b: &str, d: &Diff, out: Option<&Path>, threshold: f64) -> Self {
		Self {
			a: a.to_string(),
			b: b.to_string(),
			width: d.width,
			height: d.height,
			compared: d.compared(),
			differing: d.differing,
			percent: d.percent(),
			bbox: d.bbox.map(|b| b.to_array()),
			out: out.map(|p| p.display().to_string()),
			threshold,
			failed: d.exceeds(threshold),
		}
	}

	pub fn to_json(&self) -> Result<String> {
		Ok(serde_json::to_string_pretty(self)?)
	}

	pub fn to_text(&self) -> String {
		let head = if self.differing == 0 {
			format!("identical — {}x{}, 0 pixels differ", self.width, self.height)
		} else {
			let bbox = self
				.bbox
				.map(|[x1, y1, x2, y2]| format!(" — bbox [{x1},{y1}][{x2},{y2}]"))
				.unwrap_or_default();
			format!(
				"{} of {} pixels differ ({:.3}%){bbox}",
				self.differing, self.compared, self.percent
			)
		};
		let verdict = format!(
			"threshold {:.3}%: {}",
			self.threshold,
			if self.failed { "FAIL" } else { "PASS" }
		);
		let overlay = self.out.as_ref().map(|o| format!("overlay: {o}")).unwrap_or_default();
		[head, verdict, overlay]
			.into_iter()
			.filter(|l| !l.is_empty())
			.collect::<Vec<_>>()
			.join("\n")
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	use crate::diff::{BBox, Diff};

	fn changed() -> Diff {
		Diff {
			width: 100,
			height: 100,
			differing: 100,
			bbox: Some(BBox { x1: 5, y1: 7, x2: 15, y2: 9 }),
		}
	}

	fn identical() -> Diff {
		Diff { width: 100, height: 100, differing: 0, bbox: None }
	}

	#[test]
	fn text_reports_the_counts_bbox_and_verdict() {
		let r = Report::new("a.png", "b.png", &changed(), None, 0.5);
		let t = r.to_text();
		assert!(t.contains("100 of 10000 pixels differ (1.000%)"), "{t}");
		assert!(t.contains("bbox [5,7][15,9]"), "{t}");
		assert!(t.contains("threshold 0.500%: FAIL"), "{t}");
		// Without --out, no overlay line.
		assert!(!t.contains("overlay"), "{t}");
	}

	#[test]
	fn text_reports_identical_and_pass() {
		let r = Report::new("a.png", "b.png", &identical(), None, 0.0);
		let t = r.to_text();
		assert!(t.contains("identical"), "{t}");
		assert!(t.contains("threshold 0.000%: PASS"), "{t}");
	}

	#[test]
	fn text_mentions_the_overlay_path() {
		let r = Report::new("a.png", "b.png", &identical(), Some(Path::new("/tmp/d.png")), 0.0);
		assert!(r.to_text().contains("overlay: /tmp/d.png"), "{}", r.to_text());
	}

	#[test]
	fn json_carries_every_field_and_nulls_the_bbox_when_identical() {
		let r = Report::new("a.png", "b.png", &changed(), Some(Path::new("d.png")), 0.0);
		let v: serde_json::Value = serde_json::from_str(&r.to_json().unwrap()).unwrap();
		assert_eq!(v["a"], "a.png");
		assert_eq!(v["compared"], 10000);
		assert_eq!(v["differing"], 100);
		assert_eq!(v["bbox"], serde_json::json!([5, 7, 15, 9]));
		assert_eq!(v["out"], "d.png");
		assert_eq!(v["failed"], true);

		let same = Report::new("a.png", "b.png", &identical(), None, 0.0);
		let v: serde_json::Value = serde_json::from_str(&same.to_json().unwrap()).unwrap();
		assert_eq!(v["bbox"], serde_json::Value::Null);
		assert_eq!(v["failed"], false);
	}
}
