//! `uitool pick x y` — the color under a point, as `#AARRGGBB`.
//!
//! A screencap is always opaque, so the alpha byte is `FF` in practice;
//! the full 8-digit form is what Android tooling speaks and what the
//! source site documents, so it is what comes out.

use anyhow::{bail, Context, Result};
use image::{DynamicImage, GenericImageView};

use adbkit::Adb;

use crate::adb;

/// Color at (x, y) of a fresh device capture.
pub fn pick(dev: &Adb, x: i32, y: i32) -> Result<String> {
	let png = adb::screencap(dev).context("screencap")?;
	let img = image::load_from_memory(&png).context("decode screencap PNG")?;
	color_at(&img, x, y)
}

/// `#AARRGGBB` at (x, y). Out-of-screen coordinates are an error, never
/// a clamped neighbour pixel: picking the wrong pixel silently is worse
/// than failing loudly.
pub fn color_at(img: &DynamicImage, x: i32, y: i32) -> Result<String> {
	let (w, h) = img.dimensions();
	if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
		bail!("({x},{y}) is outside the screen ({w}x{h})");
	}
	let p = img.get_pixel(x as u32, y as u32);
	Ok(format!("#{:02X}{:02X}{:02X}{:02X}", p.0[3], p.0[0], p.0[1], p.0[2]))
}

#[cfg(test)]
mod tests {
	use super::*;

	/// The source site's opacity → hex-alpha table (不透明度16进制值),
	/// kept verbatim as the formatting fixture: alpha = round(pct ×
	/// 255 / 100). It pins the `#AARRGGBB` slot order and the uppercase
	/// two-digit hex against a reference every Android dev recognizes.
	const ALPHA_TABLE: &[(u8, &str)] = &[
		(0, "00"), (5, "0D"), (10, "1A"), (15, "26"), (20, "33"), (25, "40"),
		(30, "4D"), (35, "59"), (40, "66"), (45, "73"), (50, "80"), (55, "8C"),
		(60, "99"), (65, "A6"), (70, "B3"), (75, "BF"), (80, "CC"), (85, "D9"),
		(90, "E6"), (95, "F2"), (100, "FF"),
	];

	#[test]
	fn alpha_slot_matches_the_source_sites_hex_table() {
		for &(pct, hex) in ALPHA_TABLE {
			let a = (f64::from(pct) * 255.0 / 100.0).round() as u8;
			assert_eq!(format!("{a:02X}"), hex, "table rule at {pct}%");

			let mut img = image::RgbaImage::new(1, 1);
			img.put_pixel(0, 0, image::Rgba([0x12, 0x34, 0x56, a]));
			let out = color_at(&DynamicImage::ImageRgba8(img), 0, 0).unwrap();
			assert_eq!(out, format!("#{hex}123456"), "at {pct}%");
		}
	}

	#[test]
	fn rgb_screencap_pixel_comes_back_opaque() {
		let mut i = image::RgbImage::new(100, 50);
		i.put_pixel(7, 9, image::Rgb([0xDE, 0xAD, 0xBE]));
		let out = color_at(&DynamicImage::ImageRgb8(i), 7, 9).unwrap();
		assert_eq!(out, "#FFDEADBE");
	}

	#[test]
	fn outside_the_screen_is_an_error_naming_the_size() {
		let img = DynamicImage::ImageRgb8(image::RgbImage::new(1080, 1920));
		for (x, y) in [(1080, 0), (0, 1920), (-1, 0), (0, -1)] {
			let err = color_at(&img, x, y).unwrap_err();
			assert!(err.to_string().contains("1080x1920"), "{x},{y}: {err}");
		}
	}
}
