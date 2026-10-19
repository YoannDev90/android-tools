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
	Some(Bounds {
		x1: nums[0],
		y1: nums[1],
		x2: nums[2],
		y2: nums[3],
	})
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
						u32::from_str_radix(hex, 16).ok().and_then(char::from_u32).map(|c| c.to_string())
					} else {
						ent.strip_prefix("&#").and_then(|e| e.strip_suffix(';')).and_then(|e| e.parse::<u32>().ok()).and_then(char::from_u32).map(|c| c.to_string())
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
		nodes.push(Node {
			text,
			desc,
			clickable,
			bounds: parse_bounds(&bounds),
		});
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
