//! Pure generation core — density math, resizing, round masks, 9-patch
//! rescaling and adaptive-icon XML. Everything is in-memory (a `Vec<Asset>`
//! describes the files), so the whole pipeline tests without touching the
//! filesystem.

use image::imageops::FilterType;
use image::{DynamicImage, Rgba, RgbaImage};

/// The five Android densities: bucket name and scale over mdpi.
pub const DENSITIES: [(&str, f32); 5] =
	[("mdpi", 1.0), ("hdpi", 1.5), ("xhdpi", 2.0), ("xxhdpi", 3.0), ("xxxhdpi", 4.0)];

/// dp bases per mode.
pub const LAUNCHER_DP: u32 = 48;
pub const NOTIFICATION_DP: u32 = 24;
pub const ADAPTIVE_DP: u32 = 108;

const MARKER: Rgba<u8> = Rgba([0, 0, 0, 255]);

/// One planned output file: a PNG layer or a text resource, living in a
/// res/ subdirectory (`mipmap-xxhdpi`, `drawable-mdpi`, `values`…).
pub struct Asset {
	pub dir: String,
	pub file: String,
	pub image: Option<RgbaImage>,
	pub text: Option<String>,
}

/// dp → px at a density, with the platform's rounding.
pub fn dp_px(dp: u32, density: f32) -> u32 {
	(dp as f32 * density).round() as u32
}

/// `(density, px)` table for a dp base: `size_table(48)` is the launcher grid.
pub fn size_table(dp: u32) -> Vec<(&'static str, u32)> {
	DENSITIES.iter().map(|&(name, d)| (name, dp_px(dp, d))).collect()
}

/// Android resource name rules: `[a-z][a-z0-9_]*`.
pub fn check_name(name: &str) -> Result<(), String> {
	let ok = name.starts_with(|c: char| c.is_ascii_lowercase())
		&& name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
	ok.then_some(())
		.ok_or_else(|| format!("invalid resource name {name:?}, expected [a-z][a-z0-9_]*"))
}

/// `#RGB`, `#RRGGBB` or `#AARRGGBB` → straight RGBA.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
	let hex = s.strip_prefix('#').ok_or_else(|| format!("color {s:?} must start with #"))?;
	if !hex.is_ascii() {
		return Err(format!("color {s:?} is not ASCII hex"));
	}
	if hex.len() == 3 {
		let nibble = |i: usize| {
			u8::from_str_radix(&hex[i..i + 1], 16).map(|n| n * 16 + n).map_err(|e| e.to_string())
		};
		return Ok(Rgba([nibble(0)?, nibble(1)?, nibble(2)?, 255]));
	}
	let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string());
	match hex.len() {
		6 => Ok(Rgba([byte(0)?, byte(2)?, byte(4)?, 255])),
		8 => Ok(Rgba([byte(2)?, byte(4)?, byte(6)?, byte(0)?])),
		_ => Err(format!("color {s:?}: use #RGB, #RRGGBB or #AARRGGBB")),
	}
}

/// Resize a square source to one density. Non-square input is refused:
/// every icon asset this tool emits is square by contract.
pub fn resize_square(source: &DynamicImage, size: u32) -> Result<RgbaImage, String> {
	let (w, h) = (source.width(), source.height());
	if w != h {
		return Err(format!("source is {w}x{h}, expected a square image"));
	}
	Ok(source.resize_exact(size, size, FilterType::Lanczos3).to_rgba8())
}

/// Circle-mask a square layer — Asset Studio's "round" launcher icon.
pub fn circle_mask(layer: &RgbaImage) -> RgbaImage {
	let n = layer.width() as f64;
	let center = (n - 1.0) / 2.0;
	let radius = n / 2.0;
	let mut out = layer.clone();
	for (x, y, p) in out.enumerate_pixels_mut() {
		let (dx, dy) = (x as f64 - center, y as f64 - center);
		if (dx * dx + dy * dy).sqrt() > radius {
			*p = Rgba([0, 0, 0, 0]);
		}
	}
	out
}

/// The v26 adaptive-icon entry; only the background reference changes
/// between a color and a PNG layer.
pub fn adaptive_xml(name: &str, background: &str) -> String {
	let open = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n";
	let bg = format!("    <background android:drawable=\"{background}\" />\n");
	let fg = format!("    <foreground android:drawable=\"@drawable/{name}_foreground\" />\n");
	format!("{open}{bg}{fg}</adaptive-icon>\n")
}

pub fn color_xml(name: &str, hex: &str) -> String {
	let open = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n";
	let color = format!("    <color name=\"{name}_background\">{hex}</color>\n");
	format!("{open}{color}</resources>\n")
}

/// Legacy launcher icons (`mipmap-*`), optionally with circle-cropped
/// round variants.
pub fn launcher(
	source: &DynamicImage,
	name: &str,
	round: Option<&DynamicImage>,
) -> Result<Vec<Asset>, String> {
	check_name(name)?;
	let mut assets = Vec::new();
	for (density, size) in size_table(LAUNCHER_DP) {
		assets.push(Asset {
			dir: format!("mipmap-{density}"),
			file: format!("{name}.png"),
			image: Some(resize_square(source, size)?),
			text: None,
		});
	}
	if let Some(round) = round {
		for (density, size) in size_table(LAUNCHER_DP) {
			assets.push(Asset {
				dir: format!("mipmap-{density}"),
				file: format!("{name}_round.png"),
				image: Some(circle_mask(&resize_square(round, size)?)),
				text: None,
			});
		}
	}
	Ok(assets)
}

/// Notification icons (`drawable-*`, 24 dp, white-on-transparent by
/// convention — the system tints them).
pub fn notification(source: &DynamicImage, name: &str) -> Result<Vec<Asset>, String> {
	check_name(name)?;
	Ok(size_table(NOTIFICATION_DP)
		.into_iter()
		.map(|(density, size)| Asset {
			dir: format!("drawable-{density}"),
			file: format!("{name}.png"),
			image: Some(resize_square(source, size).expect("square checked by caller")),
			text: None,
		})
		.collect())
}

/// Generic drawable at a caller-chosen dp base.
pub fn drawable(source: &DynamicImage, name: &str, dp: u32) -> Result<Vec<Asset>, String> {
	check_name(name)?;
	Ok(size_table(dp)
		.into_iter()
		.map(|(density, size)| Asset {
			dir: format!("drawable-{density}"),
			file: format!("{name}.png"),
			image: Some(resize_square(source, size).expect("square checked by caller")),
			text: None,
		})
		.collect())
}

pub enum Background {
	/// `#RRGGBB` — emitted as a color resource.
	Color(String),
	Image(DynamicImage),
}

/// Adaptive icon: 108 dp foreground + background layer + the v26 entry
/// points. Foreground/background layers land in `drawable-*`, the entry
/// in `mipmap-anydpi-v26`.
pub fn adaptive(
	source: &DynamicImage,
	name: &str,
	background: Background,
) -> Result<Vec<Asset>, String> {
	check_name(name)?;
	let mut assets: Vec<Asset> = Vec::new();
	for (density, size) in size_table(ADAPTIVE_DP) {
		assets.push(Asset {
			dir: format!("drawable-{density}"),
			file: format!("{name}_foreground.png"),
			image: Some(resize_square(source, size)?),
			text: None,
		});
	}
	let background_ref = match &background {
		Background::Color(hex) => {
			parse_color(hex)?;
			assets.push(Asset {
				dir: "values".to_string(),
				file: format!("{name}_background.xml"),
				image: None,
				text: Some(color_xml(name, hex)),
			});
			format!("@color/{name}_background")
		}
		Background::Image(img) => {
			for (density, size) in size_table(ADAPTIVE_DP) {
				assets.push(Asset {
					dir: format!("drawable-{density}"),
					file: format!("{name}_background.png"),
					image: Some(resize_square(img, size)?),
					text: None,
				});
			}
			format!("@drawable/{name}_background")
		}
	};
	for entry in [format!("{name}.xml"), format!("{name}_round.xml")] {
		assets.push(Asset {
			dir: "mipmap-anydpi-v26".to_string(),
			file: entry,
			image: None,
			text: Some(adaptive_xml(name, &background_ref)),
		});
	}
	Ok(assets)
}

/// A parsed `.9.png`: the content (border stripped) plus the black marker
/// ranges, in content coordinates.
pub struct NinePatch {
	pub image: RgbaImage,
	/// `[start, end)` ranges of the top border (x) — bottom mirrors them.
	pub x_markers: Vec<(u32, u32)>,
	/// `[start, end)` ranges of the left border (y) — right mirrors them.
	pub y_markers: Vec<(u32, u32)>,
}

fn is_marker(p: &Rgba<u8>) -> bool {
	p.0[..3] == [0, 0, 0] && p.0[3] > 0
}

fn is_blank(p: &Rgba<u8>) -> bool {
	p.0[3] == 0
}

/// Consecutive marker pixels as `[start, end)` ranges; anything else than
/// marker or transparent makes the border invalid (`None`).
fn marker_ranges(line: &[Rgba<u8>]) -> Option<Vec<(u32, u32)>> {
	let mut ranges = Vec::new();
	let mut start = None;
	for (i, p) in line.iter().enumerate() {
		let i = i as u32;
		if is_marker(p) {
			start.get_or_insert(i);
		} else if !is_blank(p) {
			return None;
		} else if let Some(s) = start.take() {
			ranges.push((s, i));
		}
	}
	if let Some(s) = start {
		ranges.push((s, line.len() as u32));
	}
	Some(ranges)
}

/// Validate a `.9.png` and strip its border. Top/left must carry at
/// least one marker each; bottom/right may be blank or mirror their
/// opposite side exactly (the aapt contract).
pub fn parse_nine_patch(img: &RgbaImage) -> Result<NinePatch, String> {
	let (w, h) = (img.width(), img.height());
	if w < 3 || h < 3 {
		return Err(format!("{w}x{h} is too small for a 9-patch (3x3 minimum)"));
	}
	let at = |x: u32, y: u32| *img.get_pixel(x, y);
	for (x, y) in [(0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)] {
		if !is_blank(&at(x, y)) {
			return Err(format!("corner ({x},{y}) must be transparent"));
		}
	}
	let top: Vec<Rgba<u8>> = (0..w).map(|x| at(x, 0)).collect();
	let bottom: Vec<Rgba<u8>> = (0..w).map(|x| at(x, h - 1)).collect();
	let left: Vec<Rgba<u8>> = (0..h).map(|y| at(0, y)).collect();
	let right: Vec<Rgba<u8>> = (0..h).map(|y| at(w - 1, y)).collect();
	// Marker coordinates are content-space: drop the 1px border offset.
	let shift = |ranges: Vec<(u32, u32)>| {
		ranges.into_iter().map(|(s, e)| (s - 1, e - 1)).collect::<Vec<_>>()
	};
	let x_markers = shift(marker_ranges(&top).ok_or("top border has colored pixels")?);
	let y_markers = shift(marker_ranges(&left).ok_or("left border has colored pixels")?);
	if x_markers.is_empty() {
		return Err("top border carries no stretch markers".to_string());
	}
	if y_markers.is_empty() {
		return Err("left border carries no stretch markers".to_string());
	}
	if shift(marker_ranges(&bottom).ok_or("bottom border has colored pixels")?) != x_markers {
		return Err("bottom markers must mirror the top ones".to_string());
	}
	if shift(marker_ranges(&right).ok_or("right border has colored pixels")?) != y_markers {
		return Err("right markers must mirror the left ones".to_string());
	}
	Ok(NinePatch {
		image: image::imageops::crop_imm(img, 1, 1, w - 2, h - 2).to_image(),
		x_markers,
		y_markers,
	})
}

/// Marker ranges in output coordinates: rounding never shrinks a marker
/// below one pixel and never lets it escape the content.
fn scaled_ranges(ranges: &[(u32, u32)], factor: f64, limit: u32) -> Vec<(u32, u32)> {
	ranges
		.iter()
		.map(|&(start, end)| {
			let start = ((start as f64 * factor).round() as u32).min(limit - 1);
			let end = (((end as f64 * factor).round() as u32).max(start + 1)).min(limit);
			(start, end)
		})
		.collect()
}

/// Rescale a parsed patch by `factor`: content through Lanczos, borders
/// rebuilt 1 px wide with the markers repainted at their scaled spots
/// (and mirrored on the opposite borders, so the output is always a
/// complete `.9.png`).
pub fn scale_nine_patch(np: &NinePatch, factor: f64) -> RgbaImage {
	let (cw, ch) = (np.image.width(), np.image.height());
	let nw = ((cw as f64 * factor).round() as u32).max(1);
	let nh = ((ch as f64 * factor).round() as u32).max(1);
	let content = image::imageops::resize(&np.image, nw, nh, FilterType::Lanczos3);
	let mut out = RgbaImage::from_pixel(nw + 2, nh + 2, Rgba([0, 0, 0, 0]));
	image::imageops::replace(&mut out, &content, 1, 1);
	for (start, end) in scaled_ranges(&np.x_markers, factor, nw) {
		for x in start..end {
			out.put_pixel(1 + x, 0, MARKER);
			out.put_pixel(1 + x, nh + 1, MARKER);
		}
	}
	for (start, end) in scaled_ranges(&np.y_markers, factor, nh) {
		for y in start..end {
			out.put_pixel(0, 1 + y, MARKER);
			out.put_pixel(nw + 1, 1 + y, MARKER);
		}
	}
	out
}

/// Rescale a `.9.png` across every density except `from` (the density
/// the source was authored at — scaling it onto itself is a no-op).
pub fn nine_patch(source: &RgbaImage, name: &str, from: f32) -> Result<Vec<Asset>, String> {
	check_name(name)?;
	let np = parse_nine_patch(source)?;
	Ok(DENSITIES
		.iter()
		.filter(|&&(_, d)| (d - from).abs() > f32::EPSILON)
		.map(|&(density, d)| Asset {
			dir: format!("drawable-{density}"),
			file: format!("{name}.9.png"),
			image: Some(scale_nine_patch(&np, d as f64 / from as f64)),
			text: None,
		})
		.collect())
}

/// A debug badge: SVG re-rendered crisply at every density, or a square
/// PNG resized through the same Lanczos pipeline.
pub enum Badge {
	Svg(Vec<u8>),
	Png(RgbaImage),
}

impl Badge {
	pub fn rasterize(&self, size: u32) -> Result<RgbaImage, String> {
		match self {
			Badge::Svg(data) => render_svg(data, size),
			Badge::Png(img) => {
				if img.width() != img.height() {
					return Err(format!(
						"badge PNG is {}x{}, expected square",
						img.width(),
						img.height()
					));
				}
				Ok(image::imageops::resize(img, size, size, FilterType::Lanczos3))
			}
		}
	}
}

/// Render an SVG badge at `size`x`size`, fit-contained and centered.
/// System fonts are loaded so text-bearing badges resolve.
pub fn render_svg(data: &[u8], size: u32) -> Result<RgbaImage, String> {
	let mut fonts = usvg::fontdb::Database::new();
	fonts.load_system_fonts();
	let opt = usvg::Options { fontdb: std::sync::Arc::new(fonts), ..Default::default() };
	let tree = usvg::Tree::from_data(data, &opt).map_err(|e| e.to_string())?;
	let (w, h) = (tree.size().width(), tree.size().height());
	let scale = (size as f32 / w).min(size as f32 / h);
	let (dx, dy) = ((size as f32 - w * scale) / 2.0, (size as f32 - h * scale) / 2.0);
	let transform = tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);
	let mut pixmap = tiny_skia::Pixmap::new(size, size).ok_or("cannot allocate badge pixmap")?;
	resvg::render(&tree, transform, &mut pixmap.as_mut());
	// The pixmap is premultiplied; icon compositing wants straight alpha.
	let mut out = RgbaImage::new(size, size);
	for (i, px) in pixmap.pixels().iter().enumerate() {
		let a = px.alpha();
		let un =
			|c: u8| if a == 0 { 0 } else { (u32::from(c) * 255 / u32::from(a)).min(255) as u8 };
		let (x, y) = (i as u32 % size, i as u32 / size);
		out.put_pixel(x, y, Rgba([un(px.red()), un(px.green()), un(px.blue()), a]));
	}
	Ok(out)
}

/// Where the badge sits on the icon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Corner {
	Tl,
	Tr,
	Bl,
	Br,
}

/// Top-left of the badge slot inside the icon.
pub fn corner_offset(corner: Corner, icon: u32, badge: u32) -> (u32, u32) {
	let far = icon.saturating_sub(badge);
	match corner {
		Corner::Tl => (0, 0),
		Corner::Tr => (far, 0),
		Corner::Bl => (0, far),
		Corner::Br => (far, far),
	}
}

/// Alpha-composite the badge into one icon, in place.
pub fn apply_badge(icon: &mut RgbaImage, badge: &RgbaImage, corner: Corner) {
	let (x, y) = corner_offset(corner, icon.width(), badge.width());
	image::imageops::overlay(icon, badge, x as i64, y as i64);
}

/// Launcher icons for a debug build: the regular set (legacy + round),
/// every layer carrying the badge in the chosen corner. Meant for
/// `src/debug/res`, which AGP applies over `src/main/res` in debug
/// builds without any Gradle change.
pub fn debug(
	source: &DynamicImage,
	name: &str,
	round: Option<&DynamicImage>,
	badge: &Badge,
	corner: Corner,
	size_pct: u32,
) -> Result<Vec<Asset>, String> {
	if size_pct == 0 || size_pct > 100 {
		return Err(format!("badge size {size_pct}% out of range 1..=100"));
	}
	let mut assets = launcher(source, name, round)?;
	for asset in &mut assets {
		let Some(img) = asset.image.as_mut() else { continue };
		let size = ((u64::from(img.width()) * u64::from(size_pct)) / 100).max(1) as u32;
		let badge_img = badge.rasterize(size)?;
		apply_badge(img, &badge_img, corner);
	}
	Ok(assets)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn source(size: u32) -> DynamicImage {
		DynamicImage::ImageRgba8(RgbaImage::from_fn(size, size, |x, y| {
			Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255])
		}))
	}

	#[test]
	fn dp_tables_follow_the_density_ladder() {
		assert_eq!(
			size_table(48),
			[("mdpi", 48u32), ("hdpi", 72), ("xhdpi", 96), ("xxhdpi", 144), ("xxxhdpi", 192)]
		);
		assert_eq!(size_table(24).last().unwrap().1, 96);
		assert_eq!(
			size_table(108),
			[("mdpi", 108u32), ("hdpi", 162), ("xhdpi", 216), ("xxhdpi", 324), ("xxxhdpi", 432)]
		);
		assert_eq!(dp_px(27, 1.5), 41); // 40.5 rounds up
	}

	#[test]
	fn resource_names_follow_android_rules() {
		assert!(check_name("ic_launcher").is_ok());
		assert!(check_name("a").is_ok());
		assert!(check_name("ic_2").is_ok());
		for bad in ["", "Ic", "1abc", "a-b", "ünïcode", "ic Launcher"] {
			assert!(check_name(bad).is_err(), "{bad:?} must be refused");
		}
	}

	#[test]
	fn colors_parse_every_hex_spelling() {
		assert_eq!(parse_color("#fff"), Ok(Rgba([255, 255, 255, 255])));
		assert_eq!(parse_color("#2196F3"), Ok(Rgba([33, 150, 243, 255])));
		assert_eq!(parse_color("#80FF0000"), Ok(Rgba([255, 0, 0, 128]))); // AARRGGBB
		for bad in ["2196F3", "#ggg", "#12", "#éééé"] {
			assert!(parse_color(bad).is_err(), "{bad:?} must be refused");
		}
	}

	#[test]
	fn resize_requires_a_square_and_hits_the_target_size() {
		assert_eq!(resize_square(&source(192), 96).unwrap().dimensions(), (96, 96));
		let wide = DynamicImage::ImageRgba8(RgbaImage::from_pixel(200, 100, Rgba([0, 0, 0, 255])));
		assert!(resize_square(&wide, 96).is_err());
	}

	#[test]
	fn circle_mask_clears_the_corners_keeps_the_center() {
		let masked = circle_mask(&resize_square(&source(64), 64).unwrap());
		assert_eq!(masked.get_pixel(0, 0).0[3], 0);
		assert_eq!(masked.get_pixel(63, 63).0[3], 0);
		assert_eq!(masked.get_pixel(32, 32).0[3], 255);
	}

	#[test]
	fn launcher_plan_covers_every_density_and_round_variant() {
		let assets = launcher(&source(192), "ic_launcher", Some(&source(192))).unwrap();
		assert_eq!(assets.len(), 10);
		assert_eq!(
			(assets[0].dir.as_str(), assets[0].file.as_str()),
			("mipmap-mdpi", "ic_launcher.png")
		);
		assert_eq!(assets[4].image.as_ref().unwrap().dimensions(), (192, 192));
		assert_eq!(
			(assets[5].dir.as_str(), assets[5].file.as_str()),
			("mipmap-mdpi", "ic_launcher_round.png")
		);
		assert!(launcher(&source(192), "Bad-Name", None).is_err());
		// A small source upscales instead of failing — the CLI warns.
		assert_eq!(launcher(&source(100), "ic_launcher", None).unwrap().len(), 5);
	}

	#[test]
	fn notification_and_drawable_plans_pick_their_dirs_and_bases() {
		let n = notification(&source(96), "ic_stat_paste").unwrap();
		assert_eq!(
			(n[0].dir.as_str(), n[0].image.as_ref().unwrap().dimensions()),
			("drawable-mdpi", (24, 24))
		);
		assert_eq!(n[4].image.as_ref().unwrap().dimensions(), (96, 96));
		let d = drawable(&source(400), "bg_hero", 100).unwrap();
		assert_eq!(d[0].image.as_ref().unwrap().dimensions(), (100, 100));
		assert_eq!(d[4].image.as_ref().unwrap().dimensions(), (400, 400));
	}

	#[test]
	fn adaptive_plan_emits_layers_color_and_v26_entries() {
		let assets =
			adaptive(&source(432), "ic_launcher", Background::Color("#2196F3".into())).unwrap();
		// 5 foreground layers + values color + 2 mipmap-anydpi-v26 entries.
		assert_eq!(assets.len(), 8);
		assert_eq!(assets[0].file, "ic_launcher_foreground.png");
		assert_eq!(
			(assets[5].dir.as_str(), assets[5].file.as_str()),
			("values", "ic_launcher_background.xml")
		);
		let xml = assets[6].text.as_ref().unwrap();
		assert!(xml.contains("android:drawable=\"@color/ic_launcher_background\""));
		assert!(xml.contains("@drawable/ic_launcher_foreground"));
		assert_eq!(assets[6].file, "ic_launcher.xml");
		assert_eq!(assets[7].file, "ic_launcher_round.xml");

		let bg = adaptive(&source(432), "ic_launcher", Background::Image(source(432))).unwrap();
		// 5 fg + 5 bg layers + 2 entries.
		assert_eq!(bg.len(), 12);
		assert!(bg[10].text.as_ref().unwrap().contains("@drawable/ic_launcher_background"));
		assert!(color_xml("ic_launcher", "#2196F3")
			.contains("<color name=\"ic_launcher_background\">#2196F3</color>"));
	}

	/// A 20x20-content patch: top marker x 5..10, left marker y 4..12,
	/// both mirrored on the opposite borders.
	fn fixture_patch() -> RgbaImage {
		let mut img = RgbaImage::from_pixel(22, 22, Rgba([0, 0, 0, 0]));
		for x in 5..10 {
			img.put_pixel(1 + x, 0, MARKER);
			img.put_pixel(1 + x, 21, MARKER);
		}
		for y in 4..12 {
			img.put_pixel(0, 1 + y, MARKER);
			img.put_pixel(21, 1 + y, MARKER);
		}
		for (x, y) in [(2, 2), (11, 7), (19, 19)] {
			img.put_pixel(1 + x, 1 + y, Rgba([200, 30, 30, 255]));
		}
		img
	}

	#[test]
	fn nine_patch_parses_markers_and_strips_the_border() {
		let np = parse_nine_patch(&fixture_patch()).unwrap();
		assert_eq!(np.x_markers, [(5u32, 10)]);
		assert_eq!(np.y_markers, [(4u32, 12)]);
		assert_eq!(np.image.dimensions(), (20, 20));
	}

	#[test]
	fn nine_patch_refuses_broken_patches() {
		// No markers at all.
		let blank = RgbaImage::from_pixel(22, 22, Rgba([0, 0, 0, 0]));
		assert!(parse_nine_patch(&blank).is_err());
		// Marker on a corner.
		let mut corner = fixture_patch();
		corner.put_pixel(0, 0, MARKER);
		assert!(parse_nine_patch(&corner).is_err());
		// Colored pixel inside the border.
		let mut colored = fixture_patch();
		colored.put_pixel(15, 0, Rgba([255, 0, 0, 255]));
		assert!(parse_nine_patch(&colored).is_err());
		// Bottom not mirroring the top.
		let mut unmirrored = fixture_patch();
		unmirrored.put_pixel(3, 21, MARKER);
		assert!(parse_nine_patch(&unmirrored).is_err());
		assert!(parse_nine_patch(&RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 0]))).is_err());
	}

	#[test]
	fn scaled_patch_moves_markers_with_the_content() {
		let np = parse_nine_patch(&fixture_patch()).unwrap();
		let out = scale_nine_patch(&np, 2.0);
		assert_eq!(out.dimensions(), (42, 42));
		assert_eq!(out.get_pixel(0, 0).0[3], 0); // corners stay transparent
		assert_eq!(out.get_pixel(1 + 10, 0), &MARKER); // marker x 5..10 → 10..20
		assert_eq!(out.get_pixel(1 + 19, 0), &MARKER);
		assert_eq!(out.get_pixel(1 + 9, 0).0[3], 0);
		assert_eq!(out.get_pixel(0, 1 + 8), &MARKER); // marker y 4..12 → 8..24
		assert_eq!(out.get_pixel(41, 1 + 23), &MARKER); // right mirror
	}

	#[test]
	fn nine_patch_plan_skips_the_source_density() {
		let assets = nine_patch(&fixture_patch(), "btn_row", 2.0).unwrap();
		assert_eq!(assets.len(), 4);
		assert!(!assets.iter().any(|a| a.dir == "drawable-xhdpi"));
		assert_eq!(assets[0].dir, "drawable-mdpi");
		assert_eq!(assets[0].image.as_ref().unwrap().dimensions(), (12, 12)); // 20 content @ 0.5, +2 border
		assert_eq!(assets[3].dir, "drawable-xxxhdpi");
		assert_eq!(assets[3].image.as_ref().unwrap().dimensions(), (42, 42)); // 20 content @ 2, +2 border
		assert!(nine_patch(&fixture_patch(), "Bad", 2.0).is_err());
	}

	#[test]
	fn corner_offsets_anchor_the_badge_slot() {
		assert_eq!(corner_offset(Corner::Tl, 96, 32), (0, 0));
		assert_eq!(corner_offset(Corner::Tr, 96, 32), (64, 0));
		assert_eq!(corner_offset(Corner::Bl, 96, 32), (0, 64));
		assert_eq!(corner_offset(Corner::Br, 96, 32), (64, 64));
		// A badge larger than the icon clamps instead of underflowing.
		assert_eq!(corner_offset(Corner::Br, 32, 96), (0, 0));
	}

	#[test]
	fn apply_badge_overlays_alpha_and_keeps_the_rest() {
		let mut icon = RgbaImage::from_pixel(96, 96, Rgba([10, 20, 30, 255]));
		let badge = RgbaImage::from_pixel(32, 32, Rgba([255, 0, 0, 255]));
		apply_badge(&mut icon, &badge, Corner::Br);
		assert_eq!(icon.get_pixel(95, 95), &Rgba([255, 0, 0, 255]));
		assert_eq!(icon.get_pixel(0, 0), &Rgba([10, 20, 30, 255]));
		// Semi-transparent badge pixels blend (50% alpha).
		let mut icon = RgbaImage::from_pixel(96, 96, Rgba([0, 0, 0, 255]));
		let badge = RgbaImage::from_pixel(32, 32, Rgba([255, 255, 255, 128]));
		apply_badge(&mut icon, &badge, Corner::Tl);
		let blended = icon.get_pixel(0, 0).0;
		assert!(blended[0] > 120 && blended[0] < 136, "got {blended:?}");
	}

	#[test]
	fn badge_rasterize_resizes_png_and_refuses_non_square() {
		let square = RgbaImage::from_pixel(64, 64, Rgba([255, 0, 0, 255]));
		let out = Badge::Png(square).rasterize(32).unwrap();
		assert_eq!(out.dimensions(), (32, 32));
		let wide = RgbaImage::from_pixel(64, 32, Rgba([255, 0, 0, 255]));
		assert!(Badge::Png(wide).rasterize(32).is_err());
	}

	#[test]
	fn svg_badges_render_at_any_size() {
		let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><rect width="10" height="10" fill="#00ff00"/></svg>"##;
		let out = Badge::Svg(svg.as_bytes().to_vec()).rasterize(24).unwrap();
		assert_eq!(out.dimensions(), (24, 24));
		assert_eq!(out.get_pixel(12, 12).0[1], 255); // green fills the slot
		let bad = Badge::Svg(b"<svg".to_vec()).rasterize(24);
		assert!(bad.is_err());
	}

	#[test]
	fn debug_plan_badges_every_launcher_layer() {
		let badge = Badge::Png(RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255])));
		let assets =
			debug(&source(192), "ic_launcher", Some(&source(192)), &badge, Corner::Br, 50).unwrap();
		assert_eq!(assets.len(), 10); // 5 legacy + 5 round
		let mdpi = assets[0].image.as_ref().unwrap();
		assert_eq!(mdpi.dimensions(), (48, 48));
		// 50% of 48 = 24px badge in the bottom-right corner.
		assert_eq!(mdpi.get_pixel(47, 47), &Rgba([255, 0, 0, 255]));
		assert_eq!(mdpi.get_pixel(23, 23).0[3], 255); // badge starts at 24
		let pristine = resize_square(&source(192), 48).unwrap();
		assert_eq!(*mdpi.get_pixel(22, 22), *pristine.get_pixel(22, 22));
		let round = assets[5].image.as_ref().unwrap();
		assert_eq!(round.get_pixel(47, 47), &Rgba([255, 0, 0, 255]));
	}

	#[test]
	fn debug_rejects_bad_badge_sizes() {
		let badge = Badge::Png(RgbaImage::from_pixel(8, 8, Rgba([255, 0, 0, 255])));
		assert!(debug(&source(192), "ic_launcher", None, &badge, Corner::Br, 0).is_err());
		assert!(debug(&source(192), "ic_launcher", None, &badge, Corner::Br, 101).is_err());
	}
}
