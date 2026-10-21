use anyhow::{bail, Context, Result};

use adbkit::Adb;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
	pub x1: i32,
	pub y1: i32,
	pub x2: i32,
	pub y2: i32,
}

impl Bounds {
	pub fn cx(&self) -> i32 {
		(self.x1 + self.x2) / 2
	}

	pub fn cy(&self) -> i32 {
		(self.y1 + self.y2) / 2
	}
}

#[derive(Debug, Clone)]
pub struct Node {
	pub text: String,
	pub desc: String,
	pub clickable: bool,
	pub bounds: Option<Bounds>,
}

impl Node {
	pub fn label(&self) -> &str {
		if !self.text.is_empty() {
			&self.text
		} else {
			&self.desc
		}
	}

	pub fn matches(&self, query: &str) -> bool {
		let q = query.to_lowercase();
		!self.label().is_empty() && self.label().to_lowercase().contains(&q)
	}
}

pub fn parse_bounds(s: &str) -> Option<Bounds> {
	// [x1,y1][x2,y2]
	let mut nums = Vec::new();
	let mut cur = String::new();
	for c in s.chars() {
		if c.is_ascii_digit() || c == '-' {
			cur.push(c);
		} else if !cur.is_empty() {
			nums.push(cur.parse::<i32>().ok()?);
			cur.clear();
		}
	}
	if !cur.is_empty() {
		nums.push(cur.parse::<i32>().ok()?);
	}
	if nums.len() != 4 {
		return None;
	}
	Some(Bounds { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] })
}

fn decode_entities(s: &str) -> String {
	// uiautomator emits numeric entities for control chars; enough for matching.
	let mut out = String::with_capacity(s.len());
	let mut rest = s;
	while let Some(i) = rest.find('&') {
		out.push_str(&rest[..i]);
		rest = &rest[i..];
		if let Some(j) = rest.find(';') {
			let ent = &rest[..=j];
			let repl = match ent {
				"&amp;" => Some("&".to_string()),
				"&lt;" => Some("<".to_string()),
				"&gt;" => Some(">".to_string()),
				"&quot;" => Some("\"".to_string()),
				"&apos;" | "&#39;" => Some("'".to_string()),
				_ => {
					if let Some(hex) = ent.strip_prefix("&#x").and_then(|e| e.strip_suffix(';')) {
						u32::from_str_radix(hex, 16)
							.ok()
							.and_then(char::from_u32)
							.map(|c| c.to_string())
					} else {
						ent.strip_prefix("&#")
							.and_then(|e| e.strip_suffix(';'))
							.and_then(|e| e.parse::<u32>().ok())
							.and_then(char::from_u32)
							.map(|c| c.to_string())
					}
				}
			};
			match repl {
				Some(r) => {
					out.push_str(&r);
					rest = &rest[j + 1..];
				}
				None => {
					out.push('&');
					rest = &rest[1..];
				}
			}
		} else {
			out.push('&');
			rest = &rest[1..];
		}
	}
	out.push_str(rest);
	out
}

fn attr(node: &str, name: &str) -> String {
	// attr="value" — values may contain escaped quotes rarely; uiautomator uses &quot;.
	let pat = format!("{name}=\"");
	if let Some(i) = node.find(&pat) {
		let rest = &node[i + pat.len()..];
		if let Some(j) = rest.find('"') {
			return decode_entities(&rest[..j]);
		}
	}
	String::new()
}

pub fn parse_hierarchy(xml: &str) -> Vec<Node> {
	let mut nodes = Vec::new();
	for part in xml.split("<node ").skip(1) {
		let tag_end = part.find('>').unwrap_or(part.len());
		let tag = &part[..tag_end];
		if !tag.contains("bounds=") {
			continue;
		}
		let text = attr(tag, "text");
		let desc = attr(tag, "content-desc");
		if text.is_empty() && desc.is_empty() {
			continue;
		}
		let clickable = attr(tag, "clickable") == "true";
		let bounds = attr(tag, "bounds");
		nodes.push(Node { text, desc, clickable, bounds: parse_bounds(&bounds) });
	}
	nodes
}

/// Dump UI hierarchy from the device (via /sdcard, works reliably).
pub fn dump(dev: &Adb) -> Result<String> {
	dev.shell_timeout(
		&["uiautomator", "dump", "/sdcard/uitool.xml"],
		std::time::Duration::from_secs(8),
	)
	.context("uiautomator dump")?;
	let xml = dev.exec_out(&["cat", "/sdcard/uitool.xml"])?;
	let xml = String::from_utf8_lossy(&xml).into_owned();
	if !xml.contains("<hierarchy") {
		let preview: String = xml.chars().take(200).collect();
		bail!("dump failed (screen off or locked?): {preview}");
	}
	Ok(xml)
}

pub fn find_nodes<'a>(nodes: &'a [Node], query: &str) -> Vec<&'a Node> {
	nodes.iter().filter(|n| n.matches(query)).collect()
}

/// Best node for tapping: clickable match first, then first match.
pub fn pick_tap_target<'a>(nodes: &'a [Node], query: &str) -> Result<&'a Node> {
	let found = find_nodes(nodes, query);
	if found.is_empty() {
		bail!("no node matching {query:?}");
	}
	found
		.iter()
		.find(|n| n.clickable && n.bounds.is_some())
		.or_else(|| found.iter().find(|n| n.bounds.is_some()))
		.copied()
		.with_context(|| format!("node {query:?} has no bounds"))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn node(text: &str, desc: &str, clickable: bool, bounds: Option<Bounds>) -> Node {
		Node { text: text.into(), desc: desc.into(), clickable, bounds }
	}

	#[test]
	fn parse_bounds_reads_uiautomator_format() {
		assert_eq!(
			parse_bounds("[0,108][1080,2340]"),
			Some(Bounds { x1: 0, y1: 108, x2: 1080, y2: 2340 })
		);
		// Negative coordinates must not be mistaken for separators.
		assert_eq!(parse_bounds("[-5,-5][5,5]"), Some(Bounds { x1: -5, y1: -5, x2: 5, y2: 5 }));
	}

	#[test]
	fn parse_bounds_rejects_incomplete_or_junk() {
		assert_eq!(parse_bounds(""), None);
		assert_eq!(parse_bounds("[1,2]"), None); // only 2 numbers
		assert_eq!(parse_bounds("[a,b][c,d]"), None); // not numbers
		assert_eq!(parse_bounds("nothing here"), None);
	}

	#[test]
	fn bounds_center_is_integer_midpoint() {
		let b = Bounds { x1: 10, y1: 10, x2: 21, y2: 31 };
		assert_eq!((b.cx(), b.cy()), (15, 20));
	}

	#[test]
	fn label_prefers_text_then_desc_then_empty() {
		assert_eq!(node("Hi", "D", true, None).label(), "Hi");
		assert_eq!(node("", "D", true, None).label(), "D");
		assert_eq!(node("", "", true, None).label(), "");
	}

	#[test]
	fn matches_is_case_insensitive_and_never_matches_unlabeled() {
		let n = node("Aperçu", "", false, None);
		assert!(n.matches("ap"));
		assert!(n.matches("APERÇU"));
		assert!(!n.matches("zzz"));
		// Empty label: no query matches (contains("") would be true).
		assert!(!node("", "", false, None).matches(""));
	}

	#[test]
	fn decode_entities_handles_named_and_numeric() {
		// decode_entities is private: exercised through the module directly.
		assert_eq!(decode_entities("a &amp; b"), "a & b");
		assert_eq!(
			decode_entities("&lt;tag&gt; &quot;q&quot; &apos;s&apos; &#39;t"),
			"<tag> \"q\" 's' 't"
		);
		assert_eq!(decode_entities("&#x1F600;"), "😀");
		// Unknown / unterminated entities stay literal.
		assert_eq!(decode_entities("&bogus; &&amp;"), "&bogus; &&");
	}

	#[test]
	fn parse_hierarchy_skips_unlabeled_and_boundsless_nodes() {
		let xml = r#"<hierarchy rotation="0">
<node index="0" text="" content-desc="" class="android.widget.FrameLayout" bounds="[0,0][1080,2340]">
<node index="1" text="Bare" class="android.widget.TextView" bounds="[0,0][10,10]">
</node>
<node index="2" text="Clickable" content-desc="" clickable="true" bounds="[0,1][100,50]" />
</node>
</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		// The unlabeled FrameLayout is dropped; the two labeled nodes remain.
		assert_eq!(nodes.len(), 2, "{nodes:?}");
		assert_eq!(nodes[0].label(), "Bare");
		assert!(!nodes[0].clickable);
		assert_eq!(nodes[1].label(), "Clickable");
		assert!(nodes[1].clickable);
		assert_eq!(nodes[1].bounds, Some(Bounds { x1: 0, y1: 1, x2: 100, y2: 50 }));
	}

	#[test]
	fn parse_hierarchy_decodes_entities_and_falls_back_to_desc() {
		let xml = r#"<node text="Save &amp; exit" content-desc="Close &lt;X&gt;" clickable="false" bounds="[0,0][10,10]" />"#;
		let nodes = parse_hierarchy(xml);
		assert_eq!(nodes.len(), 1);
		assert_eq!(nodes[0].text, "Save & exit");
		assert_eq!(nodes[0].desc, "Close <X>");
		// text wins over desc.
		assert_eq!(nodes[0].label(), "Save & exit");
	}

	#[test]
	fn find_nodes_filters_by_query() {
		let nodes = vec![
			node("OK", "", true, Some(Bounds { x1: 0, y1: 0, x2: 10, y2: 10 })),
			node("Cancel", "", true, Some(Bounds { x1: 20, y1: 0, x2: 30, y2: 10 })),
		];
		assert_eq!(find_nodes(&nodes, "can").len(), 1);
		assert_eq!(find_nodes(&nodes, "CAN").len(), 1);
		assert!(find_nodes(&nodes, "nope").is_empty());
	}

	#[test]
	fn pick_tap_target_prefers_clickable() {
		let plain = node("Go", "", false, Some(Bounds { x1: 0, y1: 0, x2: 10, y2: 10 }));
		let click = node("Go", "", true, Some(Bounds { x1: 50, y1: 50, x2: 60, y2: 60 }));
		// Clickable is second in document order but must win.
		let nodes = vec![plain.clone(), click.clone()];
		let picked = pick_tap_target(&nodes, "Go").unwrap();
		assert_eq!(picked.bounds, click.bounds);

		// Without any clickable match, the first labeled node with bounds wins.
		let only_plain = [plain.clone()];
		let picked = pick_tap_target(&only_plain, "Go").unwrap();
		assert_eq!(picked.bounds, plain.bounds);
	}

	#[test]
	fn pick_tap_target_errors_when_nothing_found_or_no_bounds() {
		assert!(pick_tap_target(&[], "Go").is_err());

		let boundsless = node("Go", "", true, None);
		let err = pick_tap_target(&[boundsless], "Go").unwrap_err();
		assert!(err.to_string().contains("has no bounds"), "{err}");
	}
}
