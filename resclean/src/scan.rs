//! Definitions versus references in an Android source tree.
//!
//! A resource is defined by a file (`res/drawable-xhdpi/row.png` →
//! `drawable/row`) or by a values entry (`<string name="hello">`).
//! References come from XML (`@drawable/row`, `@id/title`, data-binding
//! expressions included — they live in the same attribute values) and from
//! code (`R.drawable.row`, `R2.drawable.row`). Defined and never
//! referenced = dead.
//!
//! Deliberate limits, all in the "never flag a live resource" direction:
//! - `tools:` attributes count as used (a preview reference keeps the
//!   resource);
//! - a resource referenced only by a dead one still counts as used (no
//!   cascade analysis);
//! - `getIdentifier()` lookups are invisible — `--keep` them;
//! - style parents (`parent="Theme.X"`) count as references.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;

/// Directories never descended into.
const SKIP_DIRS: &[&str] = &[".git", ".gradle", "build", "target", "captures", "node_modules"];

/// Values elements that define a resource by `name`.
const VALUES_DEFS: &[&str] = &[
	"string",
	"color",
	"dimen",
	"style",
	"plurals",
	"string-array",
	"integer-array",
	"bool",
	"integer",
	"fraction",
	"attr",
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Res {
	/// `drawable`, `string`, `layout`, …
	pub kind: String,
	pub name: String,
}

impl Res {
	fn key(&self) -> String {
		format!("{}/{}", self.kind, self.name)
	}
}

#[derive(Debug, Default)]
pub struct Report {
	/// kind → sorted names.
	pub dead: BTreeMap<String, Vec<String>>,
	pub defs: usize,
	pub refs: usize,
}

impl Report {
	pub fn total(&self) -> usize {
		self.dead.values().map(Vec::len).sum()
	}
}

/// A file's resource type: the directory name up to its first `-`
/// (`drawable-xhdpi` → `drawable`); `values*` maps to `values`.
fn kind_of_dir(dir: &str) -> Option<&str> {
	let base = dir.split('-').next().unwrap_or(dir);
	if base.is_empty() {
		return None;
	}
	if base == "values" || dir.starts_with("values") {
		Some("values")
	} else {
		Some(base)
	}
}

/// A file name's resource name: `row.9.png` → `row`, `row.png` → `row`.
fn name_of_file(file: &str) -> Option<&str> {
	let stem = file
		.strip_suffix(".9.png")
		.unwrap_or_else(|| Path::new(file).file_stem().and_then(|s| s.to_str()).unwrap_or(file));
	(!stem.is_empty()).then_some(stem)
}

fn is_res_file(name: &str) -> bool {
	matches!(
		Path::new(name).extension().and_then(|e| e.to_str()),
		Some("png" | "jpg" | "jpeg" | "webp" | "xml" | "gif" | "bmp")
	)
}

/// `@type/name` and `@+id/name` inside an XML attribute or text node;
/// group 1 is the `+` of a definition site.
fn xml_ref_re() -> &'static Regex {
	static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
	RE.get_or_init(|| Regex::new(r"@(\+)?([a-z][a-z0-9_]*)/([\w.]+)").expect("static regex"))
}

/// Split XML matches into definition sites (`@+id/…`) and references.
fn collect_xml_refs(value: &str, defs: &mut Vec<Res>, refs: &mut Vec<Res>) {
	for cap in xml_ref_re().captures_iter(value) {
		let res = normalize(&cap[2], &cap[3]);
		if cap.get(1).is_some() && res.kind == "id" {
			defs.push(res);
		} else {
			refs.push(res);
		}
	}
}

/// `R.type.name` / `R2.type.name` in Kotlin or Java.
fn code_ref_re() -> &'static Regex {
	static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
	RE.get_or_init(|| {
		Regex::new(r"\bR2?\.([a-z][a-z0-9_]*)\.([A-Za-z0-9_]+)").expect("static regex")
	})
}

/// Style names arrive from code with dots flattened to underscores
/// (`R.style.Theme_AppCompat` = `@style/Theme.AppCompat`).
fn normalize(kind: &str, name: &str) -> Res {
	Res {
		kind: kind.to_string(),
		name: if kind == "style" { name.replace('_', ".") } else { name.to_string() },
	}
}

/// Every file under `root`, skipping build outputs and VCS dirs.
fn walk(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
	let entries = std::fs::read_dir(root).with_context(|| format!("read {}", root.display()))?;
	for entry in entries {
		let entry = entry?;
		let path = entry.path();
		let name = entry.file_name();
		let name = name.to_string_lossy();
		if path.is_dir() {
			if !SKIP_DIRS.contains(&name.as_ref()) {
				walk(&path, out)?;
			}
		} else {
			out.push(path);
		}
	}
	Ok(())
}

/// Definitions from a file path under `res/`: the type dir plus the file
/// name. `None` for files that define nothing (`values` XML is parsed
/// separately).
fn file_def(path: &Path, res_root: &Path) -> Option<Res> {
	let rel = path.strip_prefix(res_root).ok()?;
	let dir = rel.parent()?.to_str()?;
	let file = path.file_name()?.to_str()?;
	if !is_res_file(file) {
		return None;
	}
	let kind = kind_of_dir(dir)?;
	let name = name_of_file(file)?;
	(kind != "values").then(|| Res { kind: kind.to_string(), name: name.to_string() })
}

/// Definitions and inner references of one values XML file.
fn values_xml(xml: &str, file: &str) -> Result<(Vec<Res>, Vec<Res>)> {
	let doc = roxmltree::Document::parse(xml).with_context(|| format!("parse {file}"))?;
	let mut defs = Vec::new();
	let mut refs = Vec::new();
	for el in doc.descendants().filter(|n| n.is_element()) {
		let tag = el.tag_name().name();
		if VALUES_DEFS.contains(&tag) {
			if let Some(name) = el.attribute("name") {
				defs.push(Res { kind: tag.to_string(), name: name.to_string() });
			}
		} else if tag == "item" {
			if let (Some(kind), Some(name)) = (el.attribute("type"), el.attribute("name")) {
				defs.push(Res { kind: kind.to_string(), name: name.to_string() });
			}
		}
		// A style parent is a reference even without `@`.
		if tag == "style" {
			if let Some(parent) = el.attribute("parent") {
				let parent = parent.strip_prefix("@style/").unwrap_or(parent);
				refs.push(normalize("style", parent));
			}
		}
		for attr in el.attributes() {
			if attr.name() == "parent" {
				continue;
			}
			collect_xml_refs(attr.value(), &mut defs, &mut refs);
		}
		if let Some(text) = el.text() {
			collect_xml_refs(text, &mut defs, &mut refs);
		}
	}
	Ok((defs, refs))
}

/// Definitions and references inside a non-values XML file (layout,
/// manifest, menu…). `@+id/…` defines an id, everything else references.
fn xml_defs_and_refs(xml: &str, file: &str) -> Result<(Vec<Res>, Vec<Res>)> {
	let doc = roxmltree::Document::parse(xml).with_context(|| format!("parse {file}"))?;
	let mut defs = Vec::new();
	let mut refs = Vec::new();
	for el in doc.descendants().filter(|n| n.is_element()) {
		for attr in el.attributes() {
			// tools: previews are not runtime references.
			if attr.namespace() == Some("http://schemas.android.com/tools") {
				continue;
			}
			collect_xml_refs(attr.value(), &mut defs, &mut refs);
		}
		collect_xml_refs(el.text().unwrap_or_default(), &mut defs, &mut refs);
	}
	Ok((defs, refs))
}

/// Scan `root` (an Android project or module) for dead resources.
///
/// `keep` patterns are regexes matched against `kind/name`; matching
/// resources are never reported.
pub fn scan(root: &Path, keep: &[Regex]) -> Result<Report> {
	scan_with(&all_files(root)?, keep)
}

fn all_files(root: &Path) -> Result<Vec<PathBuf>> {
	let mut files = Vec::new();
	walk(root, &mut files)?;
	Ok(files)
}

fn scan_with(files: &[PathBuf], keep: &[Regex]) -> Result<Report> {
	let mut defs: BTreeSet<Res> = BTreeSet::new();
	let mut refs: BTreeSet<Res> = BTreeSet::new();

	for path in files {
		let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
		let ext = Path::new(name).extension().and_then(|e| e.to_str()).unwrap_or_default();

		// res/ file definitions + XML references.
		if let Some(res_root) = res_root_of(path) {
			if let Some(def) = file_def(path, &res_root) {
				defs.insert(def);
			}
			if ext == "xml" {
				let xml = std::fs::read_to_string(path)
					.with_context(|| format!("read {}", path.display()))?;
				let dir_name = path
					.parent()
					.and_then(|d| d.file_name())
					.and_then(|n| n.to_str())
					.unwrap_or_default();
				if kind_of_dir(dir_name) == Some("values") {
					let (d, r) = values_xml(&xml, name)?;
					defs.extend(d);
					refs.extend(r);
				} else {
					let (d, r) = xml_defs_and_refs(&xml, name)?;
					defs.extend(d);
					refs.extend(r);
				}
			}
		} else if matches!(ext, "kt" | "java") {
			let src = std::fs::read_to_string(path)
				.with_context(|| format!("read {}", path.display()))?;
			for cap in code_ref_re().captures_iter(&src) {
				refs.insert(normalize(&cap[1], &cap[2]));
			}
		} else if name == "AndroidManifest.xml" {
			let xml = std::fs::read_to_string(path)
				.with_context(|| format!("read {}", path.display()))?;
			let (d, r) = xml_defs_and_refs(&xml, name)?;
			defs.extend(d);
			refs.extend(r);
		}
	}

	// `attr` is never reported: it is referenced through the `app:`
	// namespace in XML, which no text scan sees reliably.
	let mut report = Report { defs: defs.len(), refs: refs.len(), ..Default::default() };
	for def in &defs {
		// `attr` is referenced through the `app:` namespace in XML — a
		// text scan cannot see it reliably, so never report one.
		if def.kind == "attr" || refs.contains(def) {
			continue;
		}
		let key = def.key();
		if keep.iter().any(|re| re.is_match(&key)) {
			continue;
		}
		report.dead.entry(def.kind.clone()).or_default().push(def.name.clone());
	}
	for names in report.dead.values_mut() {
		names.sort();
	}
	Ok(report)
}

/// The `res/` root containing this file, if any.
fn res_root_of(path: &Path) -> Option<PathBuf> {
	let mut cur = path.parent()?;
	loop {
		if cur.file_name()? == "res" {
			return Some(cur.to_path_buf());
		}
		cur = cur.parent()?;
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn dir_and_file_names_become_keys() {
		assert_eq!(kind_of_dir("drawable-xhdpi"), Some("drawable"));
		assert_eq!(kind_of_dir("values-night-v8"), Some("values"));
		assert_eq!(kind_of_dir("layout-sw600dp"), Some("layout"));
		assert_eq!(name_of_file("row.9.png"), Some("row"));
		assert_eq!(name_of_file("row.png"), Some("row"));
		assert_eq!(name_of_file("activity_main.xml"), Some("activity_main"));
	}

	#[test]
	fn style_names_flatten_dots_from_code() {
		let r = normalize("style", "Theme_AppCompat");
		assert_eq!(r, Res { kind: "style".into(), name: "Theme.AppCompat".into() });
		let r = normalize("drawable", "ic_star");
		assert_eq!(r.name, "ic_star");
	}

	#[test]
	fn values_xml_defines_and_references() {
		let xml = r#"<resources>
			<string name="hello">Hello</string>
			<color name="brand">#f00</color>
			<style name="AppTheme" parent="Theme.MaterialComponents">
				<item name="android:colorBackground">@color/brand</item>
			</style>
			<item type="id" name="menu_save"/>
		</resources>"#;
		let (defs, refs) = values_xml(xml, "values.xml").unwrap();
		assert!(defs.contains(&Res { kind: "string".into(), name: "hello".into() }));
		assert!(defs.contains(&Res { kind: "id".into(), name: "menu_save".into() }));
		// The item references @color/brand; the style references its parent.
		assert!(refs.contains(&Res { kind: "color".into(), name: "brand".into() }));
		assert!(refs.iter().any(|r| r.kind == "style" && r.name.contains("MaterialComponents")));
	}

	#[test]
	fn tools_namespace_is_not_a_reference() {
		let xml = r#"<LinearLayout xmlns:android="http://schemas.android.com/apk/res/android"
			xmlns:tools="http://schemas.android.com/tools">
			<TextView android:text="@string/live" tools:text="@string/preview"/>
		</LinearLayout>"#;
		let (_, refs) = xml_defs_and_refs(xml, "layout.xml").unwrap();
		assert!(refs.contains(&Res { kind: "string".into(), name: "live".into() }));
		assert!(!refs.iter().any(|r| r.name == "preview"));
	}

	#[test]
	fn code_refs_match_r_and_r2() {
		let src = "val x = R.drawable.a; val y = R2.string.b; z(R.style.Theme_AppCompat)";
		let found: Vec<Res> =
			code_ref_re().captures_iter(src).map(|c| normalize(&c[1], &c[2])).collect();
		assert!(found.contains(&Res { kind: "drawable".into(), name: "a".into() }));
		assert!(found.contains(&Res { kind: "string".into(), name: "b".into() }));
		assert!(found.contains(&Res { kind: "style".into(), name: "Theme.AppCompat".into() }));
	}

	#[test]
	fn dead_resources_of_a_tiny_project() {
		let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny");
		let report = scan(&dir, &[]).unwrap();
		let dead = |kind: &str| report.dead.get(kind).cloned().unwrap_or_default();
		// Defined in values, referenced from code and a layout: alive.
		assert!(!dead("string").contains(&"hello".to_string()));
		// Defined and never referenced: dead.
		assert_eq!(dead("string"), vec!["unused".to_string()]);
		assert_eq!(dead("drawable"), vec!["orphan".to_string()]);
		assert_eq!(dead("layout"), vec!["unused_layout".to_string()]);
	}

	#[test]
	fn keep_patterns_suppress_findings() {
		let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny");
		let keep = [Regex::new(r"^string/").unwrap()];
		let report = scan(&dir, &keep).unwrap();
		assert!(!report.dead.contains_key("string"));
		assert_eq!(report.total(), 2);
	}
}
