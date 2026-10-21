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
	/// `resource-id`, e.g. `com.foo:id/send`. Not rendered by the app, but the
	/// only thing an icon-only view exposes — without it such a node is
	/// unreachable by `uitool tap`.
	pub id: String,
	/// Whether a tap here would activate something: the node itself, or an
	/// ancestor (Android puts the flag on the row, the label on a child).
	pub clickable: bool,
	pub bounds: Option<Bounds>,
}

impl Node {
	/// What `find` matches and `dump` prints: the text, else the description,
	/// else the resource-id.
	pub fn label(&self) -> &str {
		if !self.text.is_empty() {
			&self.text
		} else if !self.desc.is_empty() {
			&self.desc
		} else {
			&self.id
		}
	}

	/// Whether a human can read this node, i.e. it has a `text` or a
	/// `content-desc` — deliberately *not* counting `resource-id`.
	///
	/// This is what tells "nothing on this screen" from "screen off": the
	/// keyguard exposes ids only, so a screen whose every node lacks a
	/// visible label is the locked/off case.
	pub fn has_visible_label(&self) -> bool {
		!self.text.is_empty() || !self.desc.is_empty()
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

/// Parse the `<hierarchy>` XML into the nodes worth acting on.
///
/// A node is kept when it carries at least a `text`, a `content-desc` or a
/// `resource-id`.
///
/// [`Node::clickable`] reports whether a tap on the node would land on
/// something clickable — the node itself *or* any of its ancestors. Android
/// puts the flag on the row (a `LinearLayout`) while the label sits on a child
/// `TextView`, so a labeled node is almost never clickable on its own: on a
/// real Settings screen all 21 labels have `clickable="false"` yet each has a
/// clickable ancestor. Reading only the node itself made `--clickable` reject
/// every one of them.
pub fn parse_hierarchy(xml: &str) -> Vec<Node> {
	const OPEN: &str = "<node ";
	const CLOSE: &str = "</node>";

	let mut nodes = Vec::new();
	// `clickable` of each still-open ancestor, innermost last. Self-closing
	// nodes (`<node ... />`) never open anything, so they must not be pushed.
	let mut ancestors: Vec<bool> = Vec::new();
	let mut rest = xml;

	loop {
		let open = rest.find(OPEN);
		let close = rest.find(CLOSE);
		match (open, close) {
			(None, None) => break,
			(None, Some(c)) => {
				ancestors.pop();
				rest = &rest[c + CLOSE.len()..];
			}
			// A close tag before the next open one ends an ancestor.
			(Some(o), Some(c)) if c < o => {
				ancestors.pop();
				rest = &rest[c + CLOSE.len()..];
			}
			(Some(o), _) => {
				let after = &rest[o + OPEN.len()..];
				let tag_end = after.find('>').unwrap_or(after.len());
				let tag = &after[..tag_end];
				let clickable = attr(tag, "clickable") == "true";

				// `<` and `&` are escaped by the serializer, so neither
				// `<node ` nor `</node>` can appear inside an attribute value,
				// and no `>` does either (`&gt;` has none) — scanning for the
				// first `>` is therefore safe.
				if tag.contains("bounds=") {
					let text = attr(tag, "text");
					let desc = attr(tag, "content-desc");
					let id = attr(tag, "resource-id");
					if !(text.is_empty() && desc.is_empty() && id.is_empty()) {
						let bounds = attr(tag, "bounds");
						nodes.push(Node {
							text,
							desc,
							id,
							clickable: clickable || ancestors.iter().any(|&c| c),
							bounds: parse_bounds(&bounds),
						});
					}
				}
				if !tag.trim_end().ends_with('/') {
					ancestors.push(clickable);
				}
				rest = &after[tag_end..];
			}
		}
	}
	nodes
}

/// [`parse_hierarchy`], but refusing a hierarchy in which no node carries
/// something a human can read.
///
/// That is exactly what `uiautomator dump` answers when the screen is off or
/// locked: it dumps the keyguard, whose nodes expose a `resource-id` but no
/// `text`/`content-desc`. Left unchecked, `uitool dump` printed nothing and
/// exited 0, and `find` reported "no node matching" while the real cause was
/// the screen state.
///
/// Nodes holding only a [`Node::id`] do not count: they are legitimate on an
/// icon-only screen, and requiring a *visible* label keeps this test working
/// once ids became addressable.
///
/// Consequence, by design: a screen whose every node is icon-only is refused
/// too. `uitool` drives the UI through labels, so such a screen offers it
/// nothing to act on either way; the common case — icon-only nodes *within* a
/// screen that has text — keeps working.
pub fn parse_hierarchy_checked(xml: &str) -> Result<Vec<Node>> {
	let nodes = parse_hierarchy(xml);
	if !nodes.iter().any(Node::has_visible_label) {
		bail!("no readable node in the hierarchy — screen off or locked? (`uitool wake`)");
	}
	Ok(nodes)
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
		Node { text: text.into(), desc: desc.into(), id: String::new(), clickable, bounds }
	}

	/// Same, with a `resource-id` — what an icon-only view exposes.
	fn with_id(text: &str, desc: &str, id: &str, clickable: bool, bounds: Option<Bounds>) -> Node {
		Node { text: text.into(), desc: desc.into(), id: id.into(), clickable, bounds }
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

	/// Verbatim excerpt of what `uiautomator dump` returns on a locked, dozing
	/// screen (Redmi 23124RA7EO, Android 15): a complete hierarchy, but every
	/// node is unlabeled.
	const KEYGUARD: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?><hierarchy rotation="0"><node index="0" text="" resource-id="com.android.systemui:id/legacy_window_root" class="android.widget.FrameLayout" package="com.android.systemui" content-desc="" checkable="false" checked="false" clickable="false" enabled="true" focusable="false" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[0,0][1080,2400]"><node index="0" text="" resource-id="com.android.systemui:id/scrim_behind" class="android.view.View" content-desc="" clickable="false" bounds="[0,0][1080,2400]" /><node index="1" text="" resource-id="com.android.systemui:id/keyguard_root_view" class="android.view.ViewGroup" content-desc="" clickable="false" bounds="[0,0][1080,2400]"><node index="0" text="" resource-id="com.android.systemui:id/keyguard_message_area_container" class="android.widget.LinearLayout" content-desc="" clickable="false" bounds="[0,94][1080,2400]" /></node></node></hierarchy>"#;

	#[test]
	fn keyguard_nodes_survive_parsing_but_are_rejected_by_the_guard() {
		// The keyguard is well-formed and full of nodes — each with a
		// resource-id and nothing a human can read. Ids alone must not let it
		// pass as a usable screen.
		let raw = parse_hierarchy(KEYGUARD);
		assert!(!raw.is_empty(), "the keyguard does expose nodes, by id");
		assert!(raw.iter().all(|n| !n.has_visible_label()));
		let err = parse_hierarchy_checked(KEYGUARD).unwrap_err();
		assert!(err.to_string().contains("screen off"), "{err}");
		assert!(err.to_string().contains("uitool wake"), "{err}");
	}

	#[test]
	fn checked_parser_passes_a_labeled_hierarchy() {
		let xml = r#"<hierarchy rotation="0"><node index="0" text="Wi-Fi" clickable="false" bounds="[0,0][10,10]" /></hierarchy>"#;
		let nodes = parse_hierarchy_checked(xml).unwrap();
		assert_eq!(nodes.len(), 1);
		assert_eq!(nodes[0].label(), "Wi-Fi");
	}

	#[test]
	fn checked_parser_rejects_an_all_icon_screen() {
		// Documented limitation: `uitool` acts on labels, so a hierarchy whose
		// every node is icon-only is refused like a locked screen. The useful
		// case — icon-only nodes living on a screen that *does* have text, as
		// on Settings — is covered by `id_only_nodes_are_kept_on_a_text_screen`.
		let xml = r#"<hierarchy rotation="0"><node index="0" text="" content-desc="" resource-id="com.foo:id/send" clickable="true" bounds="[0,0][10,10]" /></hierarchy>"#;
		let err = parse_hierarchy_checked(xml).unwrap_err();
		assert!(err.to_string().contains("screen off"), "{err}");
	}

	#[test]
	fn id_only_nodes_are_kept_on_a_text_screen() {
		// The 61 such nodes counted on a real Settings screen: icon-only views
		// that were completely unreachable before ids were parsed, while the
		// screen itself stays usable thanks to its labeled rows.
		let xml = r#"<hierarchy rotation="0"><node index="0" text="Wi-Fi" clickable="false" bounds="[0,0][10,10]" /><node index="1" text="" content-desc="" resource-id="com.foo:id/search_action" clickable="true" bounds="[20,20][60,60]" /></hierarchy>"#;
		let nodes = parse_hierarchy_checked(xml).unwrap();
		assert_eq!(nodes.len(), 2);
		// Addressable now, where before only the labeled row was.
		assert!(nodes.iter().any(|n| n.matches("search_action")));
	}

	/// Shape captured on a real Settings screen: the row carries the flag, the
	/// label sits on a child TextView two levels below.
	const SETTINGS_ROW: &str = r#"<hierarchy rotation="0">
<node index="0" class="android.widget.FrameLayout" clickable="false" bounds="[0,0][1080,2400]">
<node index="0" class="android.widget.LinearLayout" clickable="false" bounds="[25,826][1055,943]">
<node index="0" text="" class="android.widget.LinearLayout" clickable="true" bounds="[25,826][1055,943]">
<node index="0" text="Wi-Fi" class="android.widget.TextView" clickable="false" bounds="[150,862][235,907]" />
<node index="1" text="On" class="android.widget.TextView" clickable="false" bounds="[955,866][983,903]" />
</node>
</node>
</hierarchy>"#;

	#[test]
	fn clickable_comes_from_the_row_not_the_label() {
		let nodes = parse_hierarchy(SETTINGS_ROW);
		assert_eq!(nodes.len(), 2, "{nodes:?}");
		// Neither label is clickable itself; both are two levels under the
		// clickable row, so tapping them activates it.
		assert!(nodes.iter().all(|n| n.clickable), "{nodes:?}");
		// Both labels are visible, so the screen passes the screen-off guard.
		assert!(nodes.iter().all(Node::has_visible_label));
	}

	#[test]
	fn clickable_stays_false_without_a_clickable_ancestor() {
		let xml = r#"<node index="0" text="Title" clickable="false" bounds="[0,0][10,10]" />"#;
		let nodes = parse_hierarchy(xml);
		assert!(!nodes[0].clickable);
	}

	#[test]
	fn self_closing_node_does_not_open_an_ancestor() {
		// `<node ... />` must not leave a phantom level on the stack,
		// otherwise every later sibling inherits a bogus clickable ancestor.
		let xml = r#"<node index="0" class="ViewGroup" clickable="true" bounds="[0,0][10,10]"><node index="0" class="View" clickable="true" bounds="[0,0][1,1]" /><node index="1" text="Loose" clickable="false" bounds="[2,2][3,3]" /></node>"#;
		let nodes = parse_hierarchy(xml);
		// Only "Loose" is labeled: inside the clickable root, hence clickable.
		assert_eq!(nodes.len(), 1);
		assert!(nodes[0].clickable);

		// Same node as a sibling *outside* any clickable root.
		let xml = r#"<node index="0" class="View" clickable="true" bounds="[0,0][1,1]" /><node index="1" text="Loose" clickable="false" bounds="[2,2][3,3]" />"#;
		let nodes = parse_hierarchy(xml);
		assert_eq!(nodes.len(), 1);
		assert!(!nodes[0].clickable, "a self-closing sibling must not inherit");
	}

	#[test]
	fn id_only_node_is_kept_and_addressable() {
		// Before ids were parsed, an icon-only clickable view was invisible to
		// `find`/`tap` — 61 such nodes on a real Settings screen.
		let xml = r#"<node index="0" text="" content-desc="" resource-id="com.foo:id/send" clickable="true" bounds="[0,0][10,10]" />"#;
		let nodes = parse_hierarchy(xml);
		assert_eq!(nodes.len(), 1, "{nodes:?}");
		assert_eq!(nodes[0].id, "com.foo:id/send");
		// The id is the last-resort label, so it becomes searchable.
		assert_eq!(nodes[0].label(), "com.foo:id/send");
		assert!(nodes[0].matches("send"));
	}

	#[test]
	fn text_wins_then_desc_then_id() {
		let both = with_id("Save", "Save changes", "com.foo:id/save", false, None);
		assert_eq!(both.label(), "Save");
		assert!(both.has_visible_label());

		let desc_only = with_id("", "Close", "com.foo:id/close", false, None);
		assert_eq!(desc_only.label(), "Close");
		assert!(desc_only.has_visible_label());

		let id_only = with_id("", "", "com.foo:id/icon", false, None);
		assert_eq!(id_only.label(), "com.foo:id/icon");
		assert!(!id_only.has_visible_label());
	}
}
