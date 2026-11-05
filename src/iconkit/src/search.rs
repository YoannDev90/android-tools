//! Offline search over a parsed catalog: every word must match the name,
//! a tag or a category; exact matches outrank substrings and popularity
//! breaks ties. No network, no index — 4k icons scan in a millisecond.

use crate::catalog::Icon;

/// Category and tag filters. Within one filter everything is OR-ed
/// (« any of these categories »), the two filters are AND-ed.
#[derive(Default, Debug)]
pub struct Filters<'a> {
	pub categories: &'a [String],
	pub tags: &'a [String],
}

impl Filters<'_> {
	/// Does the icon survive both filters?
	fn passes(&self, icon: &Icon) -> bool {
		let in_category = self.categories.is_empty()
			|| self
				.categories
				.iter()
				.any(|want| icon.categories.iter().any(|have| have.eq_ignore_ascii_case(want)));
		let has_tag = self.tags.is_empty()
			|| self
				.tags
				.iter()
				.any(|want| icon.tags.iter().any(|have| have.eq_ignore_ascii_case(want)));
		in_category && has_tag
	}
}

/// One search result: the icon, and how well it matched.
#[derive(Debug)]
pub struct Hit<'a> {
	pub icon: &'a Icon,
	pub score: u32,
}

/// Search `icons` for `query`. Every whitespace-separated word must hit
/// somewhere; an empty query ranks the (filtered) catalog by popularity.
/// `limit == 0` means no truncation.
pub fn search<'a>(
	icons: &'a [Icon],
	query: &str,
	filters: &Filters<'_>,
	limit: usize,
) -> Vec<Hit<'a>> {
	let words: Vec<String> = query.split_whitespace().map(|w| w.to_ascii_lowercase()).collect();
	let mut hits: Vec<Hit<'a>> = icons
		.iter()
		.filter(|icon| filters.passes(icon))
		.filter_map(|icon| {
			if words.is_empty() {
				return Some(Hit { icon, score: 0 });
			}
			let name = icon.name.to_ascii_lowercase();
			let tags: Vec<String> = icon.tags.iter().map(|t| t.to_ascii_lowercase()).collect();
			let cats: Vec<String> =
				icon.categories.iter().map(|c| c.to_ascii_lowercase()).collect();
			let mut score = 0;
			for word in &words {
				let s = word_score(&name, &tags, &cats, word);
				if s == 0 {
					return None;
				}
				score += s;
			}
			Some(Hit { icon, score })
		})
		.collect();
	hits.sort_by(|a, b| {
		b.score
			.cmp(&a.score)
			.then_with(|| b.icon.popularity.cmp(&a.icon.popularity))
			.then_with(|| a.icon.name.cmp(&b.icon.name))
	});
	if limit > 0 {
		hits.truncate(limit);
	}
	hits
}

/// Relevance of one word against one icon, from strongest to weakest
/// signal: the name itself, then tags, then categories.
fn word_score(name: &str, tags: &[String], cats: &[String], word: &str) -> u32 {
	if name == word {
		return 10_000;
	}
	if name.starts_with(word) {
		return 5_000;
	}
	if name.contains(word) {
		return 3_000;
	}
	if tags.iter().any(|t| t == word) {
		return 800;
	}
	if tags.iter().any(|t| t.contains(word)) {
		return 400;
	}
	if cats.iter().any(|c| c == word) {
		return 100;
	}
	if cats.iter().any(|c| c.contains(word)) {
		return 50;
	}
	0
}

/// Android resource name for an icon: `[a-z][a-z0-9_]*`, under a prefix
/// (`ic_` by default), with a digit-proof fallback when the prefix is
/// empty. Names like `13mp` become `ic_13mp`, never an invalid `13mp`.
pub fn resource_name(icon: &str, prefix: &str) -> String {
	let mut base = String::with_capacity(icon.len());
	for c in icon.chars() {
		let c = c.to_ascii_lowercase();
		base.push(if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' });
	}
	if base.is_empty() {
		base.push_str("icon");
	}
	let mut out = format!("{prefix}{base}");
	if out.starts_with(|c: char| c.is_ascii_digit()) {
		out.insert_str(0, "ic_");
	}
	out
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::catalog::Icon;

	fn icon(name: &str, categories: &[&str], tags: &[&str], popularity: u32) -> Icon {
		Icon {
			name: name.to_string(),
			categories: categories.iter().map(|s| s.to_string()).collect(),
			tags: tags.iter().map(|s| s.to_string()).collect(),
			popularity,
			sizes_px: vec![24],
			unsupported_families: vec![],
		}
	}

	fn fixture() -> Vec<Icon> {
		vec![
			icon("alarm", &["Actions"], &["clock", "timer", "reminder"], 4882),
			icon("alarm_add", &["Actions"], &["clock", "plus"], 1200),
			icon("alarm_on", &["Actions"], &["clock", "enabled"], 900),
			icon("schedule", &["Actions"], &["clock", "time", "timer"], 3000),
			icon("timer", &["Device"], &["clock", "countdown"], 1500),
		]
	}

	/// The exact name outranks every `alarm_*` variant, and ties fall
	/// back to popularity.
	#[test]
	fn exact_name_wins_then_prefix_then_rest() {
		let icons = fixture();
		let hits = search(&icons, "alarm", &Filters::default(), 10);
		let names: Vec<&str> = hits.iter().map(|h| h.icon.name.as_str()).collect();
		assert_eq!(names, ["alarm", "alarm_add", "alarm_on"]);
		// Strict win for the exact name; the two prefix hits tie on score
		// and are ordered by popularity (alarm_add > alarm_on).
		assert!(hits[0].score > hits[1].score);
		assert_eq!(hits[1].score, hits[2].score);
	}

	/// A tag-only match still lands — that is the whole point of a tag.
	#[test]
	fn matches_through_tags_and_categories() {
		let icons = fixture();
		let hits = search(&icons, "countdown", &Filters::default(), 10);
		assert_eq!(hits.len(), 1);
		assert_eq!(hits[0].icon.name, "timer");

		let hits = search(&icons, "device", &Filters::default(), 10);
		assert_eq!(hits.len(), 1);
		assert_eq!(hits[0].icon.name, "timer");
	}

	/// Every word must match something (AND), case-insensitively.
	#[test]
	fn multi_word_queries_are_and() {
		let icons = fixture();
		let hits = search(&icons, "ALARM clock", &Filters::default(), 10);
		let names: Vec<&str> = hits.iter().map(|h| h.icon.name.as_str()).collect();
		assert_eq!(names, ["alarm", "alarm_add", "alarm_on"]);

		assert!(search(&icons, "alarm unicorn", &Filters::default(), 10).is_empty());
	}

	/// Empty query = browse: popularity order, no score.
	#[test]
	fn empty_query_browses_by_popularity() {
		let icons = fixture();
		let hits = search(&icons, "", &Filters::default(), 3);
		let names: Vec<&str> = hits.iter().map(|h| h.icon.name.as_str()).collect();
		assert_eq!(names, ["alarm", "schedule", "timer"]);
		assert!(hits.iter().all(|h| h.score == 0));
	}

	/// `--limit 0` is the no-truncation escape hatch.
	#[test]
	fn limit_zero_keeps_everything() {
		let icons = fixture();
		assert_eq!(search(&icons, "clock", &Filters::default(), 0).len(), 5);
		assert_eq!(search(&icons, "clock", &Filters::default(), 2).len(), 2);
	}

	/// Categories OR within the filter, and the category filter ANDs
	/// with the tag filter.
	#[test]
	fn filters_or_within_and_across() {
		let icons = fixture();
		let actions = vec!["Actions".to_string()];
		let device = vec!["Device".to_string()];
		let clock = vec!["clock".to_string()];
		let nope = vec!["Maps".to_string()];

		let hits = search(&icons, "", &Filters { categories: &actions, tags: &[] }, 10);
		assert_eq!(hits.len(), 4);

		let both = Filters { categories: &actions, tags: &clock };
		let names: Vec<&str> =
			search(&icons, "", &both, 10).iter().map(|h| h.icon.name.as_str()).collect();
		assert_eq!(names, ["alarm", "schedule", "alarm_add", "alarm_on"]);

		// Device × clock only leaves timer; Maps × anything leaves nothing.
		let dev = Filters { categories: &device, tags: &clock };
		let names: Vec<&str> =
			search(&icons, "", &dev, 10).iter().map(|h| h.icon.name.as_str()).collect();
		assert_eq!(names, ["timer"]);
		let empty = Filters { categories: &nope, tags: &[] };
		assert!(search(&icons, "", &empty, 10).is_empty());
	}

	/// Filters are case-insensitive on both sides (`--category actions`).
	#[test]
	fn filters_ignore_case() {
		let icons = fixture();
		let actions = vec!["actions".to_string()];
		assert_eq!(search(&icons, "", &Filters { categories: &actions, tags: &[] }, 10).len(), 4);
	}

	/// Android name contract: lowercased, sanitized, prefixed, and a
	/// leading digit is always cured by `ic_` — even with `--prefix ''`.
	#[test]
	fn resource_names_are_valid_android_identifiers() {
		assert_eq!(resource_name("alarm", "ic_"), "ic_alarm");
		assert_eq!(resource_name("13mp", "ic_"), "ic_13mp");
		assert_eq!(resource_name("13mp", ""), "ic_13mp");
		assert_eq!(resource_name("arrow_back", "ic_"), "ic_arrow_back");
		assert_eq!(resource_name("4g Mobiledata", "ic_"), "ic_4g_mobiledata");
		assert_eq!(resource_name("A-B", "drawable_"), "drawable_a_b");
	}
}
