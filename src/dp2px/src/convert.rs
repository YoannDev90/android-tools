//! Pure px ↔ dp ↔ sp conversion — no device, no I/O.
//!
//! Android's unit story: 160 dp is the baseline width, so a screen of
//! density `d` dpi draws 1 dp as `d / 160` px (mdpi 160 = 1:1, xxhdpi
//! 480 = 3:1). sp is dp's typographic sibling: the same density factor,
//! times the user's font scale — which is why 16 sp text can render at
//! different pixel heights on two devices with the same density. The
//! scale is a ratio in every unit: it drops out of px ↔ dp, and sp → dp
//! multiplies by it (16 sp at scale 1.5 occupies 24 dp).

use anyhow::{bail, Result};
use clap::ValueEnum;

/// A length unit Android layouts speak.
#[derive(ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
	Px,
	Dp,
	Sp,
}

impl std::fmt::Display for Unit {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(match self {
			Unit::Px => "px",
			Unit::Dp => "dp",
			Unit::Sp => "sp",
		})
	}
}

/// Convert `value` from `from` to `to` at `density` dpi and `font_scale`.
///
/// Everything routes through px, the only unit the screen physically has.
pub fn convert(value: f64, from: Unit, to: Unit, density: u32, font_scale: f64) -> Result<f64> {
	if density == 0 {
		bail!("density must be > 0 dpi");
	}
	if font_scale.is_nan() || font_scale <= 0.0 {
		bail!("font scale must be > 0");
	}
	let px = to_px(value, from, density, font_scale);
	Ok(from_px(px, to, density, font_scale))
}

fn to_px(value: f64, from: Unit, density: u32, font_scale: f64) -> f64 {
	match from {
		Unit::Px => value,
		Unit::Dp => value * f64::from(density) / 160.0,
		Unit::Sp => value * f64::from(density) / 160.0 * font_scale,
	}
}

fn from_px(px: f64, to: Unit, density: u32, font_scale: f64) -> f64 {
	match to {
		Unit::Px => px,
		Unit::Dp => px * 160.0 / f64::from(density),
		Unit::Sp => px * 160.0 / f64::from(density) / font_scale,
	}
}

/// At most two decimals, no trailing zeros: `392.73`, `132`, `5`.
///
/// The decimal point blocks the trim, so integer zeros survive
/// (`100.00` → `100`, never `1`).
pub fn fmt(value: f64) -> String {
	let s = format!("{value:.2}");
	s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
	use super::*;

	fn approx(a: f64, b: f64) {
		assert!((a - b).abs() < 1e-9, "{a} != {b}");
	}

	#[test]
	fn mdpi_is_the_identity_for_dp() {
		approx(convert(48.0, Unit::Dp, Unit::Px, 160, 1.0).unwrap(), 48.0);
	}

	#[test]
	fn the_standard_density_buckets() {
		// The five buckets every Android dev has memorized: 48 dp is the
		// accessibility touch minimum, here in real pixels on each.
		for (dpi, px) in [(160, 48.0), (240, 72.0), (320, 96.0), (480, 144.0), (640, 192.0)] {
			approx(convert(48.0, Unit::Dp, Unit::Px, dpi, 1.0).unwrap(), px);
		}
	}

	#[test]
	fn px_to_dp_on_a_440dpi_screen() {
		let dp = convert(1080.0, Unit::Px, Unit::Dp, 440, 1.0).unwrap();
		approx(dp, 392.72727272727275);
	}

	#[test]
	fn round_trip_survives_any_density() {
		for dpi in [160, 240, 320, 440, 560] {
			let px = convert(48.0, Unit::Dp, Unit::Px, dpi, 1.0).unwrap();
			approx(convert(px, Unit::Px, Unit::Dp, dpi, 1.0).unwrap(), 48.0);
		}
	}

	#[test]
	fn sp_scales_with_the_font_scale() {
		// 10 sp at the default scale, then the same text with the user's
		// font size bumped 50%.
		approx(convert(10.0, Unit::Sp, Unit::Px, 160, 1.0).unwrap(), 10.0);
		approx(convert(10.0, Unit::Sp, Unit::Px, 160, 1.5).unwrap(), 15.0);
		// sp → dp keeps the font scale as a plain ratio: 10 sp at 1.5
		// occupies the space of 15 dp. It is density that cancels out.
		approx(convert(10.0, Unit::Sp, Unit::Dp, 440, 1.5).unwrap(), 15.0);
		approx(convert(10.0, Unit::Sp, Unit::Dp, 440, 1.0).unwrap(), 10.0);
	}

	#[test]
	fn same_unit_both_sides_is_identity() {
		for dpi in [160, 440] {
			approx(convert(37.5, Unit::Px, Unit::Px, dpi, 1.3).unwrap(), 37.5);
		}
	}

	#[test]
	fn zero_density_and_invalid_font_scale_are_errors() {
		assert!(convert(1.0, Unit::Dp, Unit::Px, 0, 1.0).is_err());
		assert!(convert(1.0, Unit::Dp, Unit::Px, 160, 0.0).is_err());
		assert!(convert(1.0, Unit::Dp, Unit::Px, 160, -1.0).is_err());
		assert!(convert(1.0, Unit::Dp, Unit::Px, 160, f64::NAN).is_err());
	}

	#[test]
	fn fmt_drops_trailing_zeros_but_keeps_the_integer_part() {
		assert_eq!(fmt(392.72727272727275), "392.73");
		assert_eq!(fmt(132.0), "132");
		assert_eq!(fmt(100.0), "100");
		assert_eq!(fmt(1000.0), "1000");
		assert_eq!(fmt(5.50), "5.5");
		assert_eq!(fmt(0.0), "0");
	}

	#[test]
	fn unit_display_is_the_android_abbreviation() {
		assert_eq!(format!("{} {} {}", Unit::Px, Unit::Dp, Unit::Sp), "px dp sp");
	}
}
