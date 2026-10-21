//! Pure SVG → VectorDrawable conversion. No `adb`, no filesystem, no
//! Android SDK: SVG in, XML out, warnings alongside.
//!
//! usvg already resolves inheritance, units and shapes-to-paths, and
//! keeps every element's absolute transform — the emitter just walks
//! the tree. Arcs arrive as cubic beziers (usvg flattens them), which
//! is exactly what a VectorDrawable `pathData` holds.

use anyhow::{Context, Result};
use usvg::tiny_skia_path::{PathSegment, Point, Transform};
use usvg::{
	ClipPath, Fill, FillRule, Group, LineCap, LineJoin, Node, Paint, Path, Stop, Stroke, Tree,
};

/// Conversion result: the XML, and anything the SVG asked for that a
/// VectorDrawable cannot express (reported, approximated, never fatal).
pub struct Converted {
	pub xml: String,
	pub warnings: Vec<String>,
}

/// Convert a whole SVG document into VectorDrawable XML.
pub fn convert(svg: &str) -> Result<Converted> {
	let tree = Tree::from_str(svg, &usvg::Options::default()).context("parse SVG")?;
	let w = tree.size().width();
	let h = tree.size().height();
	let mut warnings = Vec::new();
	let mut xml = format!(
		"<vector xmlns:android=\"http://schemas.android.com/apk/res/android\"\n    \
         android:width=\"{w}dp\"\n    \
         android:height=\"{h}dp\"\n    \
         android:viewportWidth=\"{w}\"\n    \
         android:viewportHeight=\"{h}\">\n"
	);
	emit_children(tree.root(), &mut xml, 1, &mut warnings);
	xml.push_str("</vector>\n");
	Ok(Converted { xml, warnings })
}

/// Everything inside a group, in document order.
fn emit_children(group: &Group, xml: &mut String, depth: usize, warnings: &mut Vec<String>) {
	for node in group.children() {
		match node {
			Node::Path(p) => emit_path(p, xml, depth, warnings),
			Node::Group(g) => emit_group(g, xml, depth, warnings),
			Node::Image(_) => warnings.push("image element skipped".into()),
			Node::Text(_) => warnings.push("text element skipped — no font support".into()),
		}
	}
}

/// A `<g>`: only interesting for its clip-path (transforms are baked
/// into the path coordinates) and its opacity (unrepresentable).
fn emit_group(g: &Group, xml: &mut String, depth: usize, warnings: &mut Vec<String>) {
	if g.opacity().get() < 1.0 {
		warnings.push(
			"group opacity is not representable in a VectorDrawable — colors keep full opacity"
				.into(),
		);
	}
	match g.clip_path() {
		// `clip-path` applies to its following siblings: it must live
		// inside a `<group>` with the paths it clips.
		Some(clip) => {
			xml.push_str(&format!("{}<group>\n", indent(depth)));
			emit_clip(clip, xml, depth + 1);
			emit_children(g, xml, depth + 1, warnings);
			xml.push_str(&format!("{}</group>\n", indent(depth)));
		}
		None => emit_children(g, xml, depth, warnings),
	}
}

/// One `clip-path` per path inside the clipPath element (and its own
/// nested clip, if any).
fn emit_clip(clip: &ClipPath, xml: &mut String, depth: usize) {
	let clip_transform = clip.transform();
	for node in clip.root().children() {
		if let Node::Path(p) = node {
			// Subroot paths carry transforms relative to the clip's own
			// root; the referencing element's transform is not in them.
			let local = p.abs_transform();
			let mapped = path_data(p.data().segments(), |pt| apply(local, clip_transform, pt));
			if !mapped.is_empty() {
				xml.push_str(&format!(
					"{}<clip-path android:pathData=\"{mapped}\"/>\n",
					indent(depth)
				));
			}
		}
	}
	if let Some(nested) = clip.clip_path() {
		emit_clip(nested, xml, depth);
	}
}

/// One `<path>`: fill, stroke, and every attribute they carry.
fn emit_path(p: &Path, xml: &mut String, depth: usize, warnings: &mut Vec<String>) {
	let local = p.abs_transform();
	let data = path_data(p.data().segments(), |pt| apply(local, Transform::identity(), pt));
	if data.is_empty() {
		return;
	}
	let fill = p.fill();
	let stroke = p.stroke();
	if fill.is_none() && stroke.is_none() {
		return;
	}
	let mut attrs = format!("\n{}    android:pathData=\"{data}\"", indent(depth));
	if let Some(f) = fill {
		let color = fill_color(f, warnings);
		attrs.push_str(&format!("\n{}    android:fillColor=\"{color}\"", indent(depth)));
		if f.rule() == FillRule::EvenOdd {
			attrs.push_str(&format!("\n{}    android:fillType=\"evenOdd\"", indent(depth)));
		}
	}
	if let Some(s) = stroke {
		stroke_attrs(&mut attrs, s, warnings, depth);
	}
	xml.push_str(&format!("{}<path{attrs}/>\n", indent(depth)));
}

/// Stroke attributes, Android flavor. Defaults (butt, miter 4) are
/// omitted — they are what the renderer does without an attribute.
fn stroke_attrs(attrs: &mut String, s: &Stroke, warnings: &mut Vec<String>, depth: usize) {
	let color = stroke_color(s, warnings);
	attrs.push_str(&format!("\n{}    android:strokeColor=\"{color}\"", indent(depth)));
	attrs.push_str(&format!(
		"\n{}    android:strokeWidth=\"{}\"",
		indent(depth),
		num(s.width().get())
	));
	match s.linecap() {
		LineCap::Butt => {}
		LineCap::Round => {
			attrs.push_str(&format!("\n{}    android:strokeLinecap=\"round\"", indent(depth)))
		}
		LineCap::Square => {
			attrs.push_str(&format!("\n{}    android:strokeLinecap=\"square\"", indent(depth)))
		}
	}
	match s.linejoin() {
		// MiterClip degrades to plain miter: it is a rendering hint,
		// not a different shape family.
		LineJoin::Miter | LineJoin::MiterClip => {
			let limit = s.miterlimit().get();
			if (limit - 4.0).abs() > f32::EPSILON {
				attrs.push_str(&format!(
					"\n{}    android:strokeMiterlimit=\"{}\"",
					indent(depth),
					num(limit)
				));
			}
		}
		LineJoin::Round => {
			attrs.push_str(&format!("\n{}    android:strokeLinejoin=\"round\"", indent(depth)))
		}
		LineJoin::Bevel => {
			attrs.push_str(&format!("\n{}    android:strokeLinejoin=\"bevel\"", indent(depth)))
		}
	}
	if let Some(dash) = s.dasharray() {
		if !dash.is_empty() {
			let list: Vec<String> = dash.iter().map(|d| num(*d)).collect();
			attrs.push_str(&format!(
				"\n{}    android:strokeDasharray=\"{}\"",
				indent(depth),
				list.join(",")
			));
			let offset = s.dashoffset();
			if offset.abs() > f32::EPSILON {
				attrs.push_str(&format!(
					"\n{}    android:strokeDashoffset=\"{}\"",
					indent(depth),
					num(offset)
				));
			}
		}
	}
}

/// The fill color as `#AARRGGBB`; gradients fall back to their first
/// stop (a visible approximation, reported).
fn fill_color(f: &Fill, warnings: &mut Vec<String>) -> String {
	let opacity = f.opacity().get();
	match f.paint() {
		Paint::Color(c) => argb(c.red, c.green, c.blue, opacity),
		Paint::LinearGradient(g) => {
			warnings.push("fill gradient is not representable — using its first stop color".into());
			gradient_fallback(g.stops(), opacity)
		}
		Paint::RadialGradient(g) => {
			warnings.push("fill gradient is not representable — using its first stop color".into());
			gradient_fallback(g.stops(), opacity)
		}
		Paint::Pattern(_) => {
			warnings.push("fill pattern is not representable — using black".into());
			argb(0, 0, 0, opacity)
		}
	}
}

/// Stroke color as `#AARRGGBB`, same gradient rule as the fill.
fn stroke_color(s: &Stroke, warnings: &mut Vec<String>) -> String {
	let opacity = s.opacity().get();
	match s.paint() {
		Paint::Color(c) => argb(c.red, c.green, c.blue, opacity),
		Paint::LinearGradient(g) => {
			warnings
				.push("stroke gradient is not representable — using its first stop color".into());
			gradient_fallback(g.stops(), opacity)
		}
		Paint::RadialGradient(g) => {
			warnings
				.push("stroke gradient is not representable — using its first stop color".into());
			gradient_fallback(g.stops(), opacity)
		}
		Paint::Pattern(_) => {
			warnings.push("stroke pattern is not representable — using black".into());
			argb(0, 0, 0, opacity)
		}
	}
}

fn gradient_fallback(stops: &[Stop], opacity: f32) -> String {
	match stops.first() {
		Some(s) => {
			let c = s.color();
			argb(c.red, c.green, c.blue, opacity * s.opacity().get())
		}
		None => argb(0, 0, 0, opacity),
	}
}

fn argb(r: u8, g: u8, b: u8, opacity: f32) -> String {
	let a = (opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
	format!("#{a:02X}{r:02X}{g:02X}{b:02X}")
}

/// A point through the given transforms, in order.
fn apply(first: Transform, second: Transform, pt: Point) -> Point {
	let mut pt = pt;
	first.map_point(&mut pt);
	second.map_point(&mut pt);
	pt
}

/// `pathData`: every point mapped through the element's absolute
/// transform, commands in SVG/Android syntax.
fn path_data<F>(segments: impl Iterator<Item = PathSegment>, map: F) -> String
where
	F: Fn(Point) -> Point,
{
	let mut parts: Vec<String> = Vec::new();
	for seg in segments {
		parts.push(match seg {
			PathSegment::MoveTo(p) => {
				let p = map(p);
				format!("M{} {}", num(p.x), num(p.y))
			}
			PathSegment::LineTo(p) => {
				let p = map(p);
				format!("L{} {}", num(p.x), num(p.y))
			}
			PathSegment::QuadTo(c, p) => {
				let (c, p) = (map(c), map(p));
				format!("Q{} {} {} {}", num(c.x), num(c.y), num(p.x), num(p.y))
			}
			PathSegment::CubicTo(c1, c2, p) => {
				let (c1, c2, p) = (map(c1), map(c2), map(p));
				format!(
					"C{} {} {} {} {} {}",
					num(c1.x),
					num(c1.y),
					num(c2.x),
					num(c2.y),
					num(p.x),
					num(p.y)
				)
			}
			PathSegment::Close => "Z".into(),
		});
	}
	parts.join(" ")
}

/// Two decimals, trailing zeros trimmed — icon-scale precision without
/// `0.30000000000000004` noise.
fn num(v: f32) -> String {
	let r = (v * 100.0).round() / 100.0;
	if r == 0.0 {
		return "0".into();
	}
	if r.fract() == 0.0 {
		return format!("{}", r as i64);
	}
	format!("{r:.2}").trim_end_matches('0').trim_end_matches('.').to_string()
}

fn indent(depth: usize) -> String {
	"    ".repeat(depth)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn convert_ok(svg: &str) -> Converted {
		convert(svg).expect("convert")
	}

	#[test]
	fn a_rect_becomes_a_closed_path() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
                 <rect width="24" height="24" fill="#ff0000"/>
               </svg>"##,
		);
		assert_eq!(
			out.xml,
			"<vector xmlns:android=\"http://schemas.android.com/apk/res/android\"\n    \
             android:width=\"24dp\"\n    \
             android:height=\"24dp\"\n    \
             android:viewportWidth=\"24\"\n    \
             android:viewportHeight=\"24\">\n    \
             <path\n        \
             android:pathData=\"M0 0 L24 0 L24 24 L0 24 Z\"\n        \
             android:fillColor=\"#FFFF0000\"/>\n\
             </vector>\n"
		);
		assert!(out.warnings.is_empty());
	}

	#[test]
	fn a_group_transform_is_baked_into_the_coordinates() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
                 <g transform="translate(5,7)">
                   <rect width="4" height="4" fill="#000"/>
                 </g>
               </svg>"##,
		);
		assert!(out.xml.contains("android:pathData=\"M5 7 L9 7 L9 11 L5 11 Z\""), "{}", out.xml);
	}

	#[test]
	fn circle_arrives_as_cubics_and_keeps_its_stroke() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16">
                 <circle cx="8" cy="8" r="4" fill="#00ff00" stroke="#00f"
                         stroke-width="1.5" stroke-linecap="round"/>
               </svg>"##,
		);
		assert!(out.xml.contains("android:fillColor=\"#FF00FF00\""));
		assert!(out.xml.contains('C'), "{}", out.xml);
		assert!(out.xml.contains("android:strokeColor=\"#FF0000FF\""));
		assert!(out.xml.contains("android:strokeWidth=\"1.5\""));
		assert!(out.xml.contains("android:strokeLinecap=\"round\""));
	}

	#[test]
	fn fill_opacity_lands_in_the_alpha_byte() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
                 <rect width="4" height="4" fill="#ff0000" fill-opacity="0.5"/>
               </svg>"##,
		);
		assert!(out.xml.contains("android:fillColor=\"#80FF0000\""), "{}", out.xml);
	}

	#[test]
	fn even_odd_fill_rule_becomes_fill_type() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
                 <path fill-rule="evenodd" fill="#123456"
                       d="M0 0h8v8H0z M2 2h4v4H2z"/>
               </svg>"##,
		);
		assert!(out.xml.contains("android:fillType=\"evenOdd\""), "{}", out.xml);
		assert!(out.xml.contains("android:fillColor=\"#FF123456\""));
	}

	#[test]
	fn stroke_dashes_joins_and_miter_are_emitted_when_non_default() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
                 <path fill="none" stroke="#000" stroke-width="2" stroke-dasharray="3 2"
                       stroke-dashoffset="1" stroke-linejoin="bevel"
                       d="M1 1h6"/>
                 <path fill="none" stroke="#000" stroke-width="2" stroke-miterlimit="8"
                       d="M1 4h6"/>
               </svg>"##,
		);
		assert!(out.xml.contains("android:strokeDasharray=\"3,2\""), "{}", out.xml);
		assert!(out.xml.contains("android:strokeDashoffset=\"1\""));
		assert!(out.xml.contains("android:strokeLinejoin=\"bevel\""));
		// miterlimit only exists on miter joins — usvg drops it on a
		// bevel path, and a plain miter join omits the default.
		assert!(out.xml.contains("android:strokeMiterlimit=\"8\""), "{}", out.xml);
		assert!(!out.xml.contains("strokeLinecap"), "butt is the default, omitted");
	}

	#[test]
	fn a_gradient_falls_back_to_its_first_stop_with_a_warning() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
                 <defs>
                   <linearGradient id="g">
                     <stop offset="0" stop-color="#112233"/>
                     <stop offset="1" stop-color="#445566"/>
                   </linearGradient>
                 </defs>
                 <rect width="8" height="8" fill="url(#g)"/>
               </svg>"##,
		);
		assert!(out.xml.contains("android:fillColor=\"#FF112233\""), "{}", out.xml);
		assert!(out.warnings.iter().any(|w| w.contains("gradient")), "{:?}", out.warnings);
	}

	#[test]
	fn a_clip_path_group_emits_clip_path_inside_group() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
                 <defs>
                   <clipPath id="c"><rect x="1" y="1" width="6" height="6"/></clipPath>
                 </defs>
                 <g clip-path="url(#c)">
                   <rect width="8" height="8" fill="#abc"/>
                 </g>
               </svg>"##,
		);
		assert!(out.xml.contains("<group>"), "{}", out.xml);
		assert!(out.xml.contains("<clip-path android:pathData=\"M1 1 L7 1 L7 7 L1 7 Z\"/>"));
		assert!(out.xml.contains("android:fillColor=\"#FFAABBCC\""));
	}

	#[test]
	fn viewport_follows_the_view_box_when_size_is_unitless() {
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">
                 <rect width="48" height="48" fill="#000"/>
               </svg>"##,
		);
		assert!(out.xml.contains("android:viewportWidth=\"48\""), "{}", out.xml);
		assert!(out.xml.contains("android:width=\"48dp\""));
	}

	#[test]
	fn group_opacity_is_reported_not_fatal() {
		// usvg drops <text> without fonts, so text never reaches the
		// emitter here — the opacity warning is the one that fires.
		let out = convert_ok(
			r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
                 <g opacity="0.5"><rect width="8" height="8" fill="#000"/></g>
               </svg>"##,
		);
		assert!(out.warnings.iter().any(|w| w.contains("opacity")), "{:?}", out.warnings);
		assert!(out.xml.contains("android:fillColor=\"#FF000000\""));
	}

	#[test]
	fn a_broken_svg_is_an_error_not_a_panic() {
		assert!(convert("not svg at all").is_err());
	}

	#[test]
	fn num_trims_noise_without_lying() {
		assert_eq!(num(24.0), "24");
		assert_eq!(num(1.5), "1.5");
		assert_eq!(num(0.333_333), "0.33");
		assert_eq!(num(-0.0), "0");
		assert_eq!(num(2.005), "2.01");
	}
}
