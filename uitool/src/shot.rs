use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};

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
	PathBuf::from(format!("/tmp/uitool-{ms}.jpg"))
}

pub fn take(serial: &str, opts: &ShotOpts) -> Result<PathBuf> {
	if opts.wake {
		adb::wake(serial)?;
	}
	let png = adb::screencap(serial).context("screencap")?;
	let img = image::load_from_memory(&png).context("decode screencap PNG")?;

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

	let out = opts.out.clone().unwrap_or_else(default_out);
	save(&img, &out, opts.quality)?;
	Ok(out)
}

fn save(img: &DynamicImage, path: &Path, quality: u8) -> Result<()> {
	if let Some(parent) = path.parent() {
		if !parent.as_os_str().is_empty() {
			std::fs::create_dir_all(parent).ok();
		}
	}
	let ext = path
		.extension()
		.and_then(|e| e.to_str())
		.unwrap_or("jpg")
		.to_ascii_lowercase();
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
