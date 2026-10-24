use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};

use adbkit::Adb;

use crate::adb;

#[derive(Debug, Default, Clone)]
pub struct ShotOpts {
	pub out: Option<PathBuf>,
	/// Crop rectangle x1,y1,x2,y2 in screenshot pixels.
	pub crop: Option<[i32; 4]>,
	/// Upscale factor after crop (icon inspection).
	pub scale: f32,
	/// Max dimension (longest side); 0 = keep.
	pub max_dim: u32,
	/// JPEG quality (ignored for png).
	pub quality: u8,
	pub wake: bool,
}

fn default_out() -> PathBuf {
	let ms = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap_or_default()
		.as_millis();
	std::env::temp_dir().join(format!("uitool-{ms}.jpg"))
}

pub fn take(dev: &Adb, opts: &ShotOpts) -> Result<PathBuf> {
	if opts.wake {
		adb::wake(dev)?;
	}
	let png = adb::screencap(dev).context("screencap")?;
	let img = image::load_from_memory(&png).context("decode screencap PNG")?;
	let img = process(img, opts)?;

	let out = opts.out.clone().unwrap_or_else(default_out);
	save(&img, &out, opts.quality)?;
	Ok(out)
}

/// Pure crop → scale → max-dim pipeline (no device): everything `take` does
/// between decoding and saving.
fn process(img: DynamicImage, opts: &ShotOpts) -> Result<DynamicImage> {
	let mut img = match opts.crop {
		Some([x1, y1, x2, y2]) => {
			let (w, h) = img.dimensions();
			let x1 = x1.clamp(0, w as i32);
			let y1 = y1.clamp(0, h as i32);
			let x2 = x2.clamp(x1, w as i32);
			let y2 = y2.clamp(y1, h as i32);
			if x2 <= x1 || y2 <= y1 {
				bail!("empty crop [{x1},{y1}][{x2},{y2}] (screen {w}x{h})");
			}
			img.crop_imm(x1 as u32, y1 as u32, (x2 - x1) as u32, (y2 - y1) as u32)
		}
		None => img,
	};

	if opts.scale > 0.0 && (opts.scale - 1.0).abs() > f32::EPSILON {
		let (w, h) = img.dimensions();
		let nw = (w as f32 * opts.scale).round().max(1.0) as u32;
		let nh = (h as f32 * opts.scale).round().max(1.0) as u32;
		img = img.resize_exact(nw, nh, FilterType::Nearest);
	}

	if opts.max_dim > 0 {
		let (w, h) = img.dimensions();
		let m = w.max(h);
		if m > opts.max_dim {
			img = img.resize(opts.max_dim, opts.max_dim, FilterType::Triangle);
		}
	}
	Ok(img)
}

fn save(img: &DynamicImage, path: &Path, quality: u8) -> Result<()> {
	if let Some(parent) = path.parent() {
		if !parent.as_os_str().is_empty() {
			std::fs::create_dir_all(parent).ok();
		}
	}
	let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("jpg").to_ascii_lowercase();
	match ext.as_str() {
		"png" => img.save_with_format(path, ImageFormat::Png)?,
		"jpg" | "jpeg" => {
			let f = std::fs::File::create(path)
				.with_context(|| format!("create {}", path.display()))?;
			let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(f, quality);
			enc.encode_image(img)?;
		}
		"webp" => img.save_with_format(path, ImageFormat::WebP)?,
		_ => img.save_with_format(path, ImageFormat::Jpeg)?,
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	/// 100x50 RGB image with a marker pixel at (0,0).
	fn img() -> DynamicImage {
		let mut i = image::RgbImage::new(100, 50);
		i.put_pixel(0, 0, image::Rgb([255, 0, 0]));
		DynamicImage::ImageRgb8(i)
	}

	fn opts() -> ShotOpts {
		ShotOpts { quality: 75, ..Default::default() }
	}

	#[test]
	fn crop_clamps_to_image_bounds() {
		let out = process(img(), &ShotOpts { crop: Some([-10, -10, 999, 999]), ..opts() }).unwrap();
		assert_eq!(out.dimensions(), (100, 50));
	}

	#[test]
	fn crop_extracts_region() {
		let out = process(img(), &ShotOpts { crop: Some([10, 5, 60, 25]), ..opts() }).unwrap();
		assert_eq!(out.dimensions(), (50, 20));
	}

	#[test]
	fn crop_empty_region_fails() {
		// x2 <= x1 after clamping.
		let err = process(img(), &ShotOpts { crop: Some([80, 5, 20, 25]), ..opts() }).unwrap_err();
		assert!(err.to_string().contains("empty crop"), "{err}");
	}

	#[test]
	fn scale_doubles_both_sides() {
		let out = process(img(), &ShotOpts { scale: 2.0, ..opts() }).unwrap();
		assert_eq!(out.dimensions(), (200, 100));
	}

	#[test]
	fn scale_of_one_is_a_noop() {
		for scale in [0.0, 1.0] {
			let out = process(img(), &ShotOpts { scale, ..opts() }).unwrap();
			assert_eq!(out.dimensions(), (100, 50), "scale {scale}");
		}
	}

	#[test]
	fn max_dim_resizes_only_when_larger() {
		let out = process(img(), &ShotOpts { max_dim: 10, ..opts() }).unwrap();
		// `resize` keeps the aspect ratio: 100x50 -> 10x5.
		assert_eq!(out.dimensions(), (10, 5));

		let out = process(img(), &ShotOpts { max_dim: 0, ..opts() }).unwrap();
		assert_eq!(out.dimensions(), (100, 50));

		let out = process(img(), &ShotOpts { max_dim: 1000, ..opts() }).unwrap();
		assert_eq!(out.dimensions(), (100, 50));
	}

	#[test]
	fn save_writes_png_and_creates_parents() {
		let dir = std::env::temp_dir().join(format!("uitool-shot-test-{}", std::process::id()));
		let path = dir.join("nested/deep/cap.png");
		save(&img(), &path, 75).unwrap();
		let back = image::open(&path).unwrap();
		assert_eq!(back.dimensions(), (100, 50));
		std::fs::remove_dir_all(&dir).ok();
	}

	#[test]
	fn save_jpg_quality_is_applied() {
		let dir = std::env::temp_dir().join(format!("uitool-quality-test-{}", std::process::id()));
		let (lo, hi) = (dir.join("lo.jpg"), dir.join("hi.jpg"));
		save(&img(), &lo, 10).unwrap();
		save(&img(), &hi, 95).unwrap();
		let (lo_len, hi_len) =
			(std::fs::metadata(&lo).unwrap().len(), std::fs::metadata(&hi).unwrap().len());
		assert!(lo_len < hi_len, "quality ignored: {lo_len} >= {hi_len}");
		assert!(image::open(&hi).is_ok());
		std::fs::remove_dir_all(&dir).ok();
	}
}
