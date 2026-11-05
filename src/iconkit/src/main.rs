//! `iconkit` — the fonts.google.com/icons shelf, on the command line.
//! The official Material Symbols catalog is parsed once and cached, so
//! search and filtering are offline and instant; only downloads touch
//! the network, and `get --vd` runs the SVG through `svg2vd` into a
//! ready-to-use `res/drawable` entry.

mod catalog;
mod net;
mod search;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Serialize;

use catalog::{Catalog, Icon, Style};
use search::{resource_name, Filters};

#[derive(Parser, Debug)]
#[command(
	name = "iconkit",
	about = "Search and download Material Symbols icons (fonts.google.com/icons)",
	version
)]
struct Cli {
	#[command(subcommand)]
	cmd: Cmd,

	/// Shape style: filled, outlined (default), rounded, sharp; repeat
	/// to search or download several at once.
	#[arg(long, value_enum, default_value = "outlined", global = true)]
	style: Vec<Style>,

	/// Re-download the catalog(s) instead of reading the local cache.
	#[arg(long, global = true)]
	refresh: bool,

	/// Catalog cache directory (default: $XDG_CACHE_HOME/iconkit).
	#[arg(long, global = true)]
	cache_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Cmd {
	/// Search the catalog — words match the name, a tag or a category.
	Search {
		/// Words; every word must match, best matches first. An empty
		/// query lists the most popular icons of the style.
		query: Vec<String>,
		/// Keep only these categories (repeatable, OR-ed).
		#[arg(long = "category", value_name = "CAT")]
		category: Vec<String>,
		/// Keep only icons carrying these tags (repeatable, OR-ed).
		#[arg(long = "tag", value_name = "TAG")]
		tag: Vec<String>,
		/// Maximum hits to print (0 = no limit).
		#[arg(long, default_value_t = 20)]
		limit: usize,
		/// Machine-readable hits on stdout.
		#[arg(long)]
		json: bool,
	},
	/// Show everything about one icon: categories, tags, sizes, URLs.
	Info {
		/// Icon name as spelled on fonts.google.com/icons.
		#[arg(value_name = "ICON")]
		name: String,
		/// Machine-readable output on stdout.
		#[arg(long)]
		json: bool,
	},
	/// List the categories of a style with their icon counts.
	Categories,
	/// Download icons: SVG files, or VectorDrawable XML with --vd.
	Get {
		/// Icon names as spelled on fonts.google.com/icons (e.g. alarm).
		#[arg(value_name = "ICON", required = true)]
		names: Vec<String>,
		/// Emit res/drawable-ready VectorDrawable XML instead of SVG.
		#[arg(long)]
		vd: bool,
		/// SVG size in px — must be one of the icon's own sizes
		/// (20/24/40/48).
		#[arg(long, default_value_t = 24)]
		size: u16,
		/// Resource-name prefix for --vd output (ic_ → ic_alarm.xml).
		#[arg(long, default_value = "ic_")]
		prefix: String,
		/// Output directory (created when missing).
		#[arg(short, long, default_value = ".")]
		out: PathBuf,
	},
}

fn main() {
	let cli = Cli::parse();
	match run(&cli) {
		// Same contract as the rest of the workspace: the code the
		// command earned (0 clean, 2 misses), 2 for any error — a
		// failure must never read as success.
		Ok(code) => std::process::exit(code),
		Err(e) => {
			eprintln!("iconkit: {e:#}");
			std::process::exit(2);
		}
	}
}

/// Subcommand dispatch; `Ok(2)` when a command completed with misses.
fn run(cli: &Cli) -> Result<i32> {
	let styles = selected_styles(&cli.style);
	match &cli.cmd {
		Cmd::Search { query, category, tag, limit, json } => {
			search_cmd(cli, &styles, query, category, tag, *limit, *json)?;
			Ok(0)
		}
		Cmd::Info { name, json } => info_cmd(cli, &styles, name, *json),
		Cmd::Categories => categories_cmd(cli, &styles),
		Cmd::Get { names, vd, size, prefix, out } => {
			let opts = GetOpts { vd: *vd, size: *size, prefix, out, multi: styles.len() > 1 };
			get_cmd(cli, &styles, names, &opts)
		}
	}
}

/// The styles to work with: the ones asked for, in order, without
/// duplicates (outlined when asked for nothing — the site's default).
fn selected_styles(arg: &[Style]) -> Vec<Style> {
	let mut styles = Vec::with_capacity(arg.len());
	for style in arg {
		if !styles.contains(style) {
			styles.push(*style);
		}
	}
	if styles.is_empty() {
		styles.push(Style::Outlined);
	}
	styles
}

/// A parsed catalog for a style: local cache first (unless `--refresh`),
/// then the network, written back to the cache for every later run.
fn load(style: Style, cli: &Cli) -> Result<Catalog> {
	let root = cli.cache_dir.clone().unwrap_or_else(catalog::cache_root);
	let path = catalog::cache_file(&root, style);
	if !cli.refresh {
		if let Some(body) = catalog::read_cache(&path) {
			match Catalog::parse(style, &body) {
				Ok(cat) => return Ok(cat),
				Err(e) => {
					eprintln!(
						"iconkit: cached {} catalog is unreadable ({e:#}), refetching",
						style.label()
					)
				}
			}
		}
	}
	let body = net::get_text(&catalog::metadata_url(style))
		.with_context(|| format!("download the {} catalog", style.family()))?;
	let cat = Catalog::parse(style, &body).context("parse downloaded icon metadata")?;
	catalog::write_cache(&path, &body)?;
	Ok(cat)
}

/// `search`: merge the selected catalogs (deduped by name), then rank.
fn search_cmd(
	cli: &Cli,
	styles: &[Style],
	query: &[String],
	category: &[String],
	tag: &[String],
	limit: usize,
	json: bool,
) -> Result<()> {
	let mut icons: Vec<Icon> = Vec::new();
	for &style in styles {
		for icon in load(style, cli)?.icons {
			if !icons.iter().any(|seen| seen.name == icon.name) {
				icons.push(icon);
			}
		}
	}
	let filters = Filters { categories: category, tags: tag };
	let hits = search::search(&icons, &query.join(" "), &filters, limit);

	if json {
		#[derive(Serialize)]
		struct Row<'a> {
			name: &'a str,
			score: u32,
			popularity: u32,
			categories: &'a [String],
			tags: &'a [String],
			sizes_px: &'a [u16],
		}
		let rows: Vec<Row> = hits
			.iter()
			.map(|h| Row {
				name: &h.icon.name,
				score: h.score,
				popularity: h.icon.popularity,
				categories: &h.icon.categories,
				tags: &h.icon.tags,
				sizes_px: &h.icon.sizes_px,
			})
			.collect();
		println!("{}", serde_json::to_string_pretty(&rows)?);
		return Ok(());
	}

	for hit in &hits {
		println!(
			"{:<22} {:<14} {}",
			hit.icon.name,
			hit.icon.categories.join(", "),
			preview(&hit.icon.tags, 6)
		);
	}
	if hits.is_empty() {
		eprintln!("iconkit: no icon matches {:?}{}", query.join(" "), hint(category, tag));
	}
	Ok(())
}

/// `info`: one row per selected style, straight from its catalog.
fn info_cmd(cli: &Cli, styles: &[Style], name: &str, json: bool) -> Result<i32> {
	#[derive(Serialize)]
	struct Row {
		style: String,
		name: String,
		categories: Vec<String>,
		tags: Vec<String>,
		popularity: u32,
		sizes_px: Vec<u16>,
		unsupported_families: Vec<String>,
		url: String,
	}

	let mut rows = Vec::with_capacity(styles.len());
	for &style in styles {
		let cat = load(style, cli)?;
		let Some(icon) = cat.get(name) else {
			eprintln!(
				"iconkit: no icon `{name}` in the {} catalog — try `iconkit search {name}`",
				style.family()
			);
			continue;
		};
		rows.push(Row {
			style: style.family().to_string(),
			name: icon.name.clone(),
			categories: icon.categories.clone(),
			tags: icon.tags.clone(),
			popularity: icon.popularity,
			sizes_px: icon.sizes_px.clone(),
			unsupported_families: icon.unsupported_families.clone(),
			url: catalog::icon_url(style, name, 24),
		});
	}
	if rows.is_empty() {
		return Ok(2);
	}
	if json {
		println!("{}", serde_json::to_string_pretty(&rows)?);
		return Ok(0);
	}
	for (i, row) in rows.iter().enumerate() {
		if i > 0 {
			println!();
		}
		println!("{} — {}", row.name, row.style);
		println!("  categories  {}", row.categories.join(", "));
		println!("  tags        {}", preview(&row.tags, 10));
		println!("  popularity  {}", row.popularity);
		println!(
			"  sizes px    {}",
			row.sizes_px.iter().map(u16::to_string).collect::<Vec<_>>().join(", ")
		);
		if !row.unsupported_families.is_empty() {
			println!("  unavailable {}", row.unsupported_families.join(", "));
		}
		println!("  svg         {}", row.url);
	}
	Ok(0)
}

/// `categories`: counts per category, biggest first.
fn categories_cmd(cli: &Cli, styles: &[Style]) -> Result<i32> {
	for (i, &style) in styles.iter().enumerate() {
		let cat = load(style, cli)?;
		let mut counts: Vec<(String, usize)> = Vec::new();
		for icon in &cat.icons {
			for category in &icon.categories {
				match counts.iter_mut().find(|(name, _)| name == category) {
					Some((_, n)) => *n += 1,
					None => counts.push((category.clone(), 1)),
				}
			}
		}
		counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
		if styles.len() > 1 {
			if i > 0 {
				println!();
			}
			println!("{}:", cat.style.family());
		}
		for (name, n) in counts {
			println!("{name:<16} {n}");
		}
	}
	Ok(0)
}

/// Everything `get` needs, bundled so `fetch_one` stays honest.
struct GetOpts<'a> {
	vd: bool,
	size: u16,
	prefix: &'a str,
	out: &'a Path,
	/// Several styles ⇒ file names carry the style suffix.
	multi: bool,
}

/// `get`: every name × every style, misses reported but never fatal —
/// one bad name must not cancel the downloads that did work.
fn get_cmd(cli: &Cli, styles: &[Style], names: &[String], opts: &GetOpts<'_>) -> Result<i32> {
	std::fs::create_dir_all(opts.out).with_context(|| format!("create {}", opts.out.display()))?;
	let mut misses = 0usize;
	for name in names {
		for &style in styles {
			if let Err(e) = fetch_one(cli, style, name, opts) {
				eprintln!("iconkit: {e:#}");
				misses += 1;
			}
		}
	}
	Ok(i32::from(misses > 0) * 2)
}

/// One icon, one style: catalog lookup, size check, download, then SVG
/// on disk or VectorDrawable XML through `svg2vd`.
fn fetch_one(cli: &Cli, style: Style, name: &str, opts: &GetOpts<'_>) -> Result<()> {
	let cat = load(style, cli)?;
	let icon = cat
		.get(name)
		.with_context(|| format!("no icon `{name}` in the {} catalog", style.family()))?;
	if !icon.sizes_px.is_empty() && !icon.sizes_px.contains(&opts.size) {
		let available = icon.sizes_px.iter().map(u16::to_string).collect::<Vec<_>>().join(", ");
		bail!("`{name}` has no {} px variant — available: {available} px", opts.size);
	}

	let url = catalog::icon_url(style, name, opts.size);
	let bytes = net::get_bytes(&url)?;
	let svg = String::from_utf8(bytes).with_context(|| format!("`{name}` SVG is not UTF-8"))?;
	if !svg.trim_start().starts_with("<svg") && !svg.trim_start().starts_with("<?xml") {
		bail!("`{name}` did not come back as an SVG document");
	}

	let suffix = if opts.multi { format!("_{}", style.label()) } else { String::new() };
	if opts.vd {
		let file = format!("{}.xml", resource_name(&format!("{name}{suffix}"), opts.prefix));
		let path = opts.out.join(&file);
		let conv = svg2vd::vd::convert(&svg)
			.with_context(|| format!("convert `{name}` into a VectorDrawable"))?;
		for warning in &conv.warnings {
			eprintln!("iconkit: {file}: {warning}");
		}
		std::fs::write(&path, &conv.xml).with_context(|| format!("write {}", path.display()))?;
		println!("wrote {}", path.display());
	} else {
		let path = opts.out.join(format!("{name}{suffix}.svg"));
		std::fs::write(&path, &svg).with_context(|| format!("write {}", path.display()))?;
		println!("wrote {}", path.display());
	}
	Ok(())
}

/// First `n` items, comma-separated, `+N` when something was cut.
fn preview(items: &[String], n: usize) -> String {
	if items.len() <= n {
		return items.join(", ");
	}
	format!("{} +{}", items[..n].join(", "), items.len() - n)
}

/// Empty-result advice: which filter to relax.
fn hint(category: &[String], tag: &[String]) -> &'static str {
	match (category.is_empty(), tag.is_empty()) {
		(false, false) => " (drop --category / --tag to widen)",
		(false, true) => " (drop --category to widen)",
		(true, false) => " (drop --tag to widen)",
		(true, true) => " — try `iconkit categories` for the vocabulary",
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Styles de-duplicate, keep order, and default to outlined.
	#[test]
	fn selected_styles_dedupes_and_defaults() {
		assert_eq!(selected_styles(&[]), [Style::Outlined]);
		assert_eq!(
			selected_styles(&[Style::Rounded, Style::Sharp, Style::Rounded]),
			[Style::Rounded, Style::Sharp]
		);
		assert_eq!(selected_styles(&[Style::Filled]), [Style::Filled]);
	}

	#[test]
	fn preview_cuts_with_a_count() {
		let items: Vec<String> = (1..=8).map(|i| format!("t{i}")).collect();
		assert_eq!(preview(&items[..3], 6), "t1, t2, t3");
		assert_eq!(preview(&items, 3), "t1, t2, t3 +5");
	}

	/// Each empty-result hint blames exactly the filters in play.
	#[test]
	fn hint_names_the_filter_to_drop() {
		let f = vec!["Actions".to_string()];
		let t = vec!["clock".to_string()];
		assert_eq!(hint(&[], &[]), " — try `iconkit categories` for the vocabulary");
		assert_eq!(hint(&f, &[]), " (drop --category to widen)");
		assert_eq!(hint(&[], &t), " (drop --tag to widen)");
		assert_eq!(hint(&f, &t), " (drop --category / --tag to widen)");
	}
}
