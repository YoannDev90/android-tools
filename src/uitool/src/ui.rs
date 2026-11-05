//! Device-side UI verbs on top of `adbkit`'s hierarchy model.
//!
//! The node model and the uiautomator XML parsing live in
//! [`adbkit::hierarchy`], shared with the tools that lint or diff the same
//! dump. What stays here is what only `uitool` needs: dumping from the
//! device, the screen-off guard, and the tap-target policy.

use anyhow::{bail, Context, Result};

use adbkit::hierarchy::{find_nodes, parse_hierarchy, Node};
use adbkit::Adb;

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

/// Best node for tapping, in decreasing order of directness:
///
/// 1. a node that **accepts the click itself** and has bounds;
/// 2. else one only tappable through an ancestor;
/// 3. else the first match with bounds at all.
///
/// Tier 1 exists because tier 2 is nearly vacuous: since an ancestor's flag
/// propagates, a toolbar title four levels under a clickable frame is
/// "tappable", and tapping it does nothing. On the Wi-Fi screen `Wi-Fi`
/// matches exactly that title, the row (self-clickable) and a switch label —
/// the title came first and was picked.
pub fn pick_tap_target<'a>(nodes: &'a [Node], query: &str) -> Result<&'a Node> {
	let found = find_nodes(nodes, query);
	if found.is_empty() {
		bail!("no node matching {query:?}");
	}
	found
		.iter()
		.find(|n| n.self_clickable && n.bounds.is_some())
		.or_else(|| found.iter().find(|n| n.clickable && n.bounds.is_some()))
		.or_else(|| found.iter().find(|n| n.bounds.is_some()))
		.copied()
		.with_context(|| format!("node {query:?} has no bounds"))
}

#[cfg(test)]
mod tests {
	use super::*;

	use adbkit::hierarchy::Bounds;

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

	/// A node whose tap only works because an ancestor accepts clicks.
	fn via_ancestor(text: &str, bounds: Option<Bounds>) -> Node {
		Node {
			text: text.into(),
			desc: String::new(),
			id: String::new(),
			self_clickable: false,
			clickable: true,
			bounds,
		}
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

	#[test]
	fn pick_tap_target_prefers_the_node_that_accepts_the_click() {
		// Captured on the Wi-Fi screen: `Wi-Fi` matches the toolbar title (only
		// tappable through a clickable frame four levels up), the row itself
		// clickable, and a switch label. The title came first in document
		// order and was picked — and tapping a title does nothing, confirmed
		// on device (focus unchanged).
		let title = via_ancestor("Wi-Fi", Some(Bounds { x1: 54, y1: 219, x2: 219, y2: 303 }));
		let row = node("Wi-Fi", "", true, Some(Bounds { x1: 25, y1: 333, x2: 1055, y2: 450 }));

		let title_first = [title.clone(), row.clone()];
		let picked = pick_tap_target(&title_first, "Wi-Fi").unwrap();
		assert_eq!(picked.bounds, row.bounds, "must pick the row, not the title");

		// Document order must not decide when a later node accepts the click.
		let row_first = [row, title];
		let picked = pick_tap_target(&row_first, "Wi-Fi").unwrap();
		assert!(picked.self_clickable);
	}

	#[test]
	fn pick_tap_target_falls_back_to_tappable_then_to_any_bounds() {
		// Nothing self-clickable: the tappable node still wins.
		let title = via_ancestor("Wi-Fi", Some(Bounds { x1: 54, y1: 219, x2: 219, y2: 303 }));
		let bare = node("Wi-Fi", "", false, Some(Bounds { x1: 0, y1: 0, x2: 10, y2: 10 }));
		let both = [bare.clone(), title.clone()];
		let picked = pick_tap_target(&both, "Wi-Fi").unwrap();
		assert_eq!(picked.bounds, title.bounds, "tappable beats a bare node");

		// Nothing clickable at all: first node with bounds.
		let only_bare = [bare];
		let picked = pick_tap_target(&only_bare, "Wi-Fi").unwrap();
		assert_eq!(picked.bounds, Some(Bounds { x1: 0, y1: 0, x2: 10, y2: 10 }));
	}

	/// Verbatim excerpt of what `uiautomator dump` returns on a locked, dozing
	/// screen (Redmi 23124RA7EO, Android 15): a complete hierarchy, but every
	/// node is unlabeled.
	const KEYGUARD: &str = r#"<?xml version='1.0' encoding='UTF-8' standalone='yes' ?><hierarchy rotation="0"><node index="0" text="" resource-id="com.android.systemui:id/legacy_window_root" class="android.widget.FrameLayout" package="com.android.systemui" content-desc="" checkable="false" checked="false" clickable="false" enabled="true" focusable="false" focused="false" scrollable="false" long-clickable="false" password="false" selected="false" bounds="[0,0][1080,2400]"><node index="0" text="" resource-id="com.android.systemui:id/scrim_behind" class="android.view.View" content-desc="" clickable="false" bounds="[0,0][1080,2400]" /><node index="1" text="" resource-id="com.android.systemui:id/keyguard_root_view" class="android.view.ViewGroup" content-desc="" clickable="false" bounds="[0,0][1080,2400]"><node index="0" text="" resource-id="com.android.systemui:id/keyguard_message_area_container" class="android.widget.LinearLayout" content-desc="" clickable="false" bounds="[0,94][1080,2400]" /></node></node></hierarchy>"#;

	#[test]
	fn keyguard_is_rejected_by_the_guard() {
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
}
