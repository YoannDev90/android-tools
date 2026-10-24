//! The catalog: Google's official Material Symbols metadata — parsed
//! once, cached on disk, searched offline from then on. The two URL
//! builders live here too, so the whole contract with fonts.google.com
//! sits behind one door.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::Deserialize;

/// The four shape styles of fonts.google.com/icons.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum Style {
	/// Solid glyphs.
	Filled,
	/// Thin outline glyphs — the site default.
	Outlined,
	/// Outline with rounded corners.
	Rounded,
	/// Outline with sharp corners.
	Sharp,
}

impl Style {
	/// Family name exactly as Google spells it in the metadata endpoint.
	pub fn family(self) -> &'static str {
		match self {
			Style::Filled => "Material Symbols",
			Style::Outlined => "Material Symbols Outlined",
			Style::Rounded => "Material Symbols Rounded",
			Style::Sharp => "Material Symbols Sharp",
		}
	}

	/// Slug of the style in the gstatic download path.
	pub fn slug(self) -> &'static str {
		match self {
			Style::Filled => "materialsymbols",
			Style::Outlined => "materialsymbolsoutlined",
			Style::Rounded => "materialsymbolsrounded",
			Style::Sharp => "materialsymbolssharp",
		}
	}

	/// Which static instance of the set to fetch: the filled cut, or the
	/// set's default (the outline). Google publishes both under one path.
	pub fn variant(self) -> &'static str {
		match self {
			Style::Filled => "fill1",
			_ => "default",
		}
	}

	/// Short label, used in file names and in file-name suffixes.
	pub fn label(self) -> &'static str {
		match self {
			Style::Filled => "filled",
			Style::Outlined => "outlined",
			Style::Rounded => "rounded",
			Style::Sharp => "sharp",
		}
	}
}

/// One icon as Google describes it.
#[derive(Clone, Debug, Deserialize)]
pub struct Icon {
	pub name: String,
	#[serde(default)]
	pub categories: Vec<String>,
	#[serde(default)]
	pub tags: Vec<String>,
	#[serde(default)]
	pub popularity: u32,
	#[serde(default)]
	pub sizes_px: Vec<u16>,
	#[serde(default)]
	pub unsupported_families: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Raw {
	#[serde(default)]
	icons: Vec<Icon>,
}

/// The icons of one style, straight from Google's metadata endpoint.
#[derive(Debug)]
pub struct Catalog {
	pub style: Style,
	pub icons: Vec<Icon>,
}

impl Catalog {
	/// Parse the endpoint's body. Google wraps the JSON in an XSSI guard
	/// (`)]}'` on its own first line) that has to come off before serde
	/// ever sees it.
	pub fn parse(style: Style, body: &str) -> Result<Catalog> {
		let json = body.strip_prefix(")]}'").map(str::trim_start).unwrap_or(body);
		let raw: Raw = serde_json::from_str(json).context("parse icon metadata")?;
		if raw.icons.is_empty() {
			bail!("icon metadata parsed but holds no icons");
		}
		Ok(Catalog { style, icons: raw.icons })
	}

	/// The icon of exactly that name, if this style has it.
	pub fn get(&self, name: &str) -> Option<&Icon> {
		self.icons.iter().find(|i| i.name == name)
	}
}

/// Official metadata endpoint of a style.
pub fn metadata_url(style: Style) -> String {
	format!("https://fonts.google.com/metadata/icons/{}", style.family().replace(' ', "%20"))
}

/// Static SVG of one icon at one size — the short-term release path that
/// fonts.google.com/icons itself serves.
pub fn icon_url(style: Style, name: &str, size: u16) -> String {
	format!(
		"https://fonts.gstatic.com/s/i/short-term/release/{}/{name}/{}/{size}px.svg",
		style.slug(),
		style.variant()
	)
}

/// Where the catalogs live: `$XDG_CACHE_HOME/iconkit`, else `~/.cache/iconkit`
/// on Unix, else `%LOCALAPPDATA%\iconkit` on Windows (absolute paths only, per
/// the spec; when no home is resolvable, a plain `iconkit/` in the working
/// directory).
pub fn cache_root() -> PathBuf {
	if cfg!(windows) {
		if let Some(base) = std::env::var_os("LOCALAPPDATA") {
			let p = PathBuf::from(&base);
			if p.is_absolute() {
				return p.join("iconkit");
			}
		}
	}
	let base = std::env::var_os("XDG_CACHE_HOME")
		.map(PathBuf::from)
		.filter(|p| p.is_absolute())
		.or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")));
	match base {
		Some(base) => base.join("iconkit"),
		None => PathBuf::from("iconkit"),
	}
}

/// The cache file of a style inside a cache root.
pub fn cache_file(root: &Path, style: Style) -> PathBuf {
	root.join(format!("{}.json", style.label()))
}

/// The raw metadata body of a cached style, if the cache holds one.
pub fn read_cache(path: &Path) -> Option<String> {
	fs::read_to_string(path).ok().filter(|body| !body.trim().is_empty())
}

/// Store the raw metadata body, creating the cache directory.
pub fn write_cache(path: &Path, body: &str) -> Result<()> {
	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
	}
	fs::write(path, body).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The XSSI guard comes off, and a real (trimmed) Google payload
	/// parses into the shape the rest of the crate walks.
	#[test]
	fn parse_strips_xssi_guard() {
		let body = ")]}'\n{\"icons\":[{\"name\":\"alarm\",\"popularity\":4882,\
		            \"unsupported_families\":[],\"categories\":[\"Actions\"],\
		            \"tags\":[\"clock\",\"timer\"],\"sizes_px\":[20,24,40,48]}]}";
		let cat = Catalog::parse(Style::Outlined, body).unwrap();
		assert_eq!(cat.icons.len(), 1);
		let icon = cat.get("alarm").unwrap();
		assert_eq!(icon.categories, ["Actions"]);
		assert_eq!(icon.sizes_px, [20, 24, 40, 48]);
		assert_eq!(icon.popularity, 4882);
		assert!(cat.get("ab_testing").is_none());
	}

	#[test]
	fn parse_rejects_empty_catalog() {
		assert!(Catalog::parse(Style::Filled, ")]}'\n{\"icons\":[]}").is_err());
	}

	#[test]
	fn parse_rejects_garbage() {
		assert!(Catalog::parse(Style::Filled, "<html>proxy error</html>").is_err());
	}

	/// Missing optional fields (a stripped payload must not panic serde).
	#[test]
	fn parse_tolerates_sparse_icons() {
		let cat = Catalog::parse(Style::Sharp, "{\"icons\":[{\"name\":\"x\"}]}").unwrap();
		assert!(cat.get("x").unwrap().tags.is_empty());
		assert_eq!(cat.get("x").unwrap().popularity, 0);
	}

	/// Both URL builders are pure string work — pinned here so an
	/// upstream path rename fails a test instead of a user's download.
	#[test]
	fn urls_match_the_live_endpoints() {
		assert_eq!(
			metadata_url(Style::Outlined),
			"https://fonts.google.com/metadata/icons/Material%20Symbols%20Outlined"
		);
		assert_eq!(
			metadata_url(Style::Filled),
			"https://fonts.google.com/metadata/icons/Material%20Symbols"
		);
		assert_eq!(
			icon_url(Style::Outlined, "alarm", 24),
			"https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsoutlined/alarm/default/24px.svg"
		);
		assert_eq!(
			icon_url(Style::Filled, "alarm", 48),
			"https://fonts.gstatic.com/s/i/short-term/release/materialsymbols/alarm/fill1/48px.svg"
		);
		assert_eq!(
			icon_url(Style::Rounded, "settings", 40),
			"https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/settings/default/40px.svg"
		);
	}

	#[test]
	fn cache_file_is_the_style_label_in_json() {
		assert_eq!(cache_file(Path::new("/c"), Style::Filled), Path::new("/c/filled.json"));
		assert_eq!(cache_file(Path::new("/c"), Style::Sharp), Path::new("/c/sharp.json"));
	}
}
