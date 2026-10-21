//! uiautomator hierarchy: the node model and its XML parsing.
//!
//! `uiautomator dump` writes a `<hierarchy>` tree of `<node>` elements; every
//! tool in this workspace that acts on the screen flattens it the same way —
//! nodes worth addressing, in document order. The model lives here so
//! `uitool` and the lint/diff tools share one definition.
//!
//! Parsing is delegated to `roxmltree`: entities, quoting and tag boundaries
//! are a real XML parser's job. The hand-rolled scanner this replaces had to
//! reason about escaped `>` and self-closing tags by hand — a tree does not.

use anyhow::{Context, Result};

use roxmltree::Document;

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
	/// The node's own `clickable` attribute, exactly as Android reported it.
	pub self_clickable: bool,
	/// Whether a tap here would activate something: the node itself, or an
	/// ancestor (Android puts the flag on the row, the label on a child).
	///
	/// Deliberately coarse — useful to *list* what a screen offers, but too
	/// loose to choose a tap target, since a toolbar title four levels below
	/// a clickable frame also counts. Tap-target policies (`uitool tap`)
	/// prefer [`Node::self_clickable`] for that reason.
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

/// Parse the `[x1,y1][x2,y2]` value of a `bounds` attribute.
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

/// Parse the `<hierarchy>` XML into the nodes worth acting on.
///
/// A node is kept when it carries at least a `text`, a `content-desc` or a
/// `resource-id`, and a `bounds` attribute (Android writes it on every node
/// it can report a position for).
///
/// [`Node::clickable`] reports whether a tap on the node would land on
/// something clickable — the node itself *or* any of its ancestors. Android
/// puts the flag on the row (a `LinearLayout`) while the label sits on a child
/// `TextView`, so a labeled node is almost never clickable on its own: on a
/// real Settings screen all 21 labels have `clickable="false"` yet each has a
/// clickable ancestor. Reading only the node itself made `--clickable` reject
/// every one of them.
///
/// A malformed document yields no node at all rather than a partial list: the
/// XML parser is all-or-nothing. See [`try_parse_hierarchy`] for the error.
pub fn parse_hierarchy(xml: &str) -> Vec<Node> {
	try_parse_hierarchy(xml).unwrap_or_default()
}

/// [`parse_hierarchy`], surfacing why nothing was parsed.
///
/// The XML layer rejects a document that is not well-formed — unknown
/// entities included, where the hand-rolled scanner this replaced passed them
/// through as literal text. A truncated `uiautomator dump` (screen off
/// mid-transition) is exactly such a document.
pub fn try_parse_hierarchy(xml: &str) -> Result<Vec<Node>> {
	let doc = Document::parse(xml).context("hierarchy is not well-formed XML")?;
	let mut nodes = Vec::new();
	for n in doc.descendants().filter(|n| n.has_tag_name("node")) {
		let text = n.attribute("text").unwrap_or_default();
		let desc = n.attribute("content-desc").unwrap_or_default();
		let id = n.attribute("resource-id").unwrap_or_default();
		if text.is_empty() && desc.is_empty() && id.is_empty() {
			continue;
		}
		if !n.has_attribute("bounds") {
			continue;
		}
		let self_clickable = n.attribute("clickable") == Some("true");
		nodes.push(Node {
			text: text.to_string(),
			desc: desc.to_string(),
			id: id.to_string(),
			// `ancestors()` starts at the node itself, so this is the node's
			// own flag OR any ancestor's.
			clickable: n.ancestors().any(|p| p.attribute("clickable") == Some("true")),
			self_clickable,
			bounds: n.attribute("bounds").and_then(parse_bounds),
		});
	}
	Ok(nodes)
}

pub fn find_nodes<'a>(nodes: &'a [Node], query: &str) -> Vec<&'a Node> {
	nodes.iter().filter(|n| n.matches(query)).collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn node(text: &str, desc: &str, clickable: bool, bounds: Option<Bounds>) -> Node {
		Node {
			text: text.into(),
			desc: desc.into(),
			id: String::new(),
			self_clickable: clickable,
			clickable,
			bounds,
		}
	}

	/// Same, with a `resource-id` — what an icon-only view exposes.
	fn with_id(text: &str, desc: &str, id: &str, clickable: bool, bounds: Option<Bounds>) -> Node {
		Node {
			text: text.into(),
			desc: desc.into(),
			id: id.into(),
			self_clickable: clickable,
			clickable,
			bounds,
		}
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
	fn entities_are_decoded_by_the_xml_parser() {
		// Named, numeric-decimal and numeric-hex entities, as uiautomator
		// emits them for control characters and as apps emit them in labels.
		let xml = r#"<node text="Save &amp; exit" content-desc="&lt;tag&gt; &quot;q&quot; &apos;s&apos; &#39;t &#x1F600;" clickable="false" bounds="[0,0][10,10]" />"#;
		let nodes = parse_hierarchy(xml);
		assert_eq!(nodes.len(), 1);
		assert_eq!(nodes[0].text, "Save & exit");
		assert_eq!(nodes[0].desc, "<tag> \"q\" 's' 't 😀");
	}

	#[test]
	fn an_unknown_entity_is_an_error_not_literal_text() {
		// Deliberate change from the hand-rolled scanner, which passed
		// `&bogus;` through verbatim: a real parser rejects the document,
		// and `try_parse_hierarchy` says so instead of silently matching on
		// entity markup.
		let err =
			try_parse_hierarchy(r#"<node text="a &bogus; b" bounds="[0,0][1,1]" />"#).unwrap_err();
		assert!(err.to_string().contains("well-formed"), "{err}");
		// The infallible wrapper stays infallible: nothing parsed, no panic.
		assert!(parse_hierarchy(r#"<node text="a &bogus; b" bounds="[0,0][1,1]" />"#).is_empty());
	}

	#[test]
	fn a_document_needs_one_root_element() {
		// The hand-rolled scanner accepted rootless fragments; a tree cannot.
		// Every real dump has a `<hierarchy>` root, so this only shows up in
		// fixtures.
		assert!(try_parse_hierarchy(
			"<node text=\"A\" bounds=\"[0,0][1,1]\" /><node text=\"B\" bounds=\"[0,0][1,1]\" />"
		)
		.is_err());
	}

	#[test]
	fn parse_hierarchy_skips_unlabeled_and_boundsless_nodes() {
		let xml = r#"<hierarchy rotation="0">
<node index="0" text="" content-desc="" class="android.widget.FrameLayout" bounds="[0,0][1080,2340]">
<node index="1" text="Bare" class="android.widget.TextView" bounds="[0,0][10,10]">
</node>
<node index="2" text="Clickable" content-desc="" clickable="true" bounds="[0,1][100,50]" />
<node index="3" text="Offscreen" class="android.widget.TextView" />
</node>
</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		// The unlabeled FrameLayout and the node without bounds are dropped.
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

	/// Verbatim excerpt of what `uiautomator dump` returns on a locked, dozing
	/// screen (Redmi 23124RA7EO, Android 15): a complete hierarchy, but every
	/// node is unlabeled.
	const KEYGUARD: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?><hierarchy rotation="0"><node index="0" text="" resource-id="com.android.systemui:id/legacy_window_root" class="android.widget.FrameLayout" package="com.android.systemui" content-desc="" checkable="false" checked="false" clickable="false" enabled="true" focusable="false" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[0,0][1080,2400]"><node index="0" text="" resource-id="com.android.systemui:id/scrim_behind" class="android.view.View" content-desc="" clickable="false" bounds="[0,0][1080,2400]" /><node index="1" text="" resource-id="com.android.systemui:id/keyguard_root_view" class="android.view.ViewGroup" content-desc="" clickable="false" bounds="[0,0][1080,2400]"><node index="0" text="" resource-id="com.android.systemui:id/keyguard_message_area_container" class="android.widget.LinearLayout" content-desc="" clickable="false" bounds="[0,94][1080,2400]" /></node></node></hierarchy>"#;

	#[test]
	fn keyguard_nodes_survive_parsing_but_have_no_visible_label() {
		// The keyguard is well-formed and full of nodes — each with a
		// resource-id and nothing a human can read. Ids alone must not count
		// as a readable screen (the guard enforcing that lives in uitool).
		let raw = parse_hierarchy(KEYGUARD);
		assert!(!raw.is_empty(), "the keyguard does expose nodes, by id");
		assert!(raw.iter().all(|n| !n.has_visible_label()));
	}

	#[test]
	fn xml_declaration_and_standalone_attribute_are_tolerated() {
		// The prolog above is what Android actually writes; parsing must not
		// choke on it.
		assert!(try_parse_hierarchy(KEYGUARD).is_ok());
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
		// The old scanner tracked open/close tags by hand, and a self-closing
		// `<node ... />` had to be kept off the ancestor stack — otherwise
		// every later sibling inherited a bogus clickable ancestor. A tree has
		// no stack to get wrong.
		let xml = r#"<hierarchy>
<node index="0" class="ViewGroup" clickable="true" bounds="[0,0][10,10]">
<node index="0" class="View" clickable="true" bounds="[0,0][1,1]" />
<node index="1" text="Inside" clickable="false" bounds="[2,2][3,3]" />
</node>
<node index="0" class="View" clickable="true" bounds="[0,0][1,1]" />
<node index="1" text="Loose" clickable="false" bounds="[2,2][3,3]" />
</hierarchy>"#;
		let nodes = parse_hierarchy(xml);
		assert_eq!(nodes.len(), 2, "{nodes:?}");
		// "Inside" sits under the clickable root, hence clickable.
		assert!(nodes[0].clickable);
		// "Loose" is a sibling of that root: the self-closing node between
		// them must not have left a phantom ancestor behind.
		assert!(!nodes[1].clickable, "a self-closing sibling must not inherit");
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
