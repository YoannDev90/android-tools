//! Pure diff core: everything between decoding and reporting — no device,
//! no files.
//!
//! Pixels are compared on their RGB triple (Euclidean distance, squared so
//! it stays integer): a screenshot pair never legitimately differs by alpha
//! alone, so alpha is deliberately ignored.

use anyhow::{bail, Context, Result};

use image::RgbaImage;

/// Inclusive bounding box of the differing pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BBox {
	pub x1: u32,
	pub y1: u32,
	pub x2: u32,
	pub y2: u32,
}

impl BBox {
	pub fn to_array(self) -> [u32; 4] {
		[self.x1, self.y1, self.x2, self.y2]
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Diff {
	/// Size of the compared area — the `--region` crop, when any.
	pub width: u32,
	pub height: u32,
	/// Pixels whose RGB differs.
	pub differing: usize,
	/// Where they are; `None` when the images are identical.
	pub bbox: Option<BBox>,
}

impl Diff {
	/// Pixels actually compared.
	pub fn compared(&self) -> usize {
		self.width as usize * self.height as usize
	}

	/// Differing pixels as a percentage of the compared area, `0.0..=100.0`.
	pub fn percent(&self) -> f64 {
		if self.compared() == 0 {
			0.0
		} else {
			self.differing as f64 * 100.0 / self.compared() as f64
		}
	}

	/// The CI contract: *strictly* above the threshold fails, so identical
	/// images pass even at `--threshold 0` (the default).
	pub fn exceeds(&self, threshold: f64) -> bool {
		self.percent() > threshold
	}
}

/// Do these pixels differ? RGB only, alpha ignored (see module docs).
#[inline]
fn differs(a: &[u8; 4], b: &[u8; 4]) -> bool {
	// Squared Euclidean distance in RGB, compared against zero: the same
	// predicate as "at least one channel differs", without a sqrt.
	let (dr, dg, db) =
		(a[0] as i32 - b[0] as i32, a[1] as i32 - b[1] as i32, a[2] as i32 - b[2] as i32);
	dr * dr + dg * dg + db * db != 0
}

/// Compare two equally sized images pixel by pixel.
pub fn diff(a: &RgbaImage, b: &RgbaImage) -> Result<Diff> {
	if a.dimensions() != b.dimensions() {
		bail!(
			"size mismatch: {}x{} vs {}x{} — compare the same screen",
			a.width(),
			a.height(),
			b.width(),
			b.height()
		);
	}
	let (width, height) = a.dimensions();
	let mut differing = 0;
	let mut bbox: Option<BBox> = None;
	for y in 0..height {
		for x in 0..width {
			if differs(&a.get_pixel(x, y).0, &b.get_pixel(x, y).0) {
				differing += 1;
				bbox = Some(match bbox {
					Some(b) => {
						BBox { x1: b.x1.min(x), y1: b.y1.min(y), x2: b.x2.max(x), y2: b.y2.max(y) }
					}
					None => BBox { x1: x, y1: y, x2: x, y2: y },
				});
			}
		}
	}
	Ok(Diff { width, height, differing, bbox })
}

/// Parse `--region x1,y1,x2,y2`. Coordinates may sit outside the image —
/// [`crop`] clamps them.
pub fn parse_region(s: &str) -> Result<[i32; 4]> {
	let nums: Vec<i32> = s
		.split(',')
		.map(|p| p.trim().parse())
		.collect::<Result<Vec<_>, _>>()
		.context("region must be x1,y1,x2,y2")?;
	if nums.len() != 4 {
		bail!("region must be x1,y1,x2,y2 (got {s:?})");
	}
	Ok([nums[0], nums[1], nums[2], nums[3]])
}

/// Crop to `region`, clamped to the image — the same semantics as
/// `uitool shot --crop`: out-of-bounds coordinates pull in, and a region
/// that ends up empty is an error rather than a silently empty comparison.
pub fn crop(img: &RgbaImage, region: [i32; 4]) -> Result<RgbaImage> {
	let (w, h) = img.dimensions();
	let [x1, y1, x2, y2] = region;
	let x1 = x1.clamp(0, w as i32);
	let y1 = y1.clamp(0, h as i32);
	let x2 = x2.clamp(x1, w as i32);
	let y2 = y2.clamp(y1, h as i32);
	if x2 <= x1 || y2 <= y1 {
		bail!("empty region [{x1},{y1}][{x2},{y2}] (image {w}x{h})");
	}
	Ok(image::imageops::crop_imm(img, x1 as u32, y1 as u32, (x2 - x1) as u32, (y2 - y1) as u32)
		.to_image())
}

const RED: image::Rgba<u8> = image::Rgba([255, 0, 0, 255]);

/// A copy of `a` with every differing pixel set to red, plus a two-pixel
/// red frame around [`Diff::bbox`]: what changed, and where, at a glance.
/// The result is the compared area — with `--region`, the crop.
pub fn overlay(a: &RgbaImage, b: &RgbaImage, d: &Diff) -> RgbaImage {
	let mut out = a.clone();
	let (w, h) = out.dimensions();
	for y in 0..h {
		for x in 0..w {
			if differs(&a.get_pixel(x, y).0, &b.get_pixel(x, y).0) {
				out.put_pixel(x, y, RED);
			}
		}
	}
	if let Some(bbox) = d.bbox {
		frame(&mut out, bbox);
	}
	out
}

/// Two-pixel-thick red border along the edges of `bbox` — thickness so it
/// still reads on a busy screenshot.
fn frame(out: &mut RgbaImage, bbox: BBox) {
	let (w, h) = out.dimensions();
	// The bbox is made of pixels inside the image; clamping only matters
	// for the second row/column of a tiny box.
	let x1 = bbox.x1.min(w - 1);
	let y1 = bbox.y1.min(h - 1);
	let x2 = bbox.x2.min(w - 1);
	let y2 = bbox.y2.min(h - 1);
	for y in y1..=y2 {
		for x in x1..=x2 {
			if x < x1 + 2 || x + 2 > x2 || y < y1 + 2 || y + 2 > y2 {
				out.put_pixel(x, y, RED);
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Uniform `w*h` image.
	fn img(w: u32, h: u32) -> RgbaImage {
		RgbaImage::from_pixel(w, h, image::Rgba([10, 20, 30, 255]))
	}

	fn is_red(p: &image::Rgba<u8>) -> bool {
		p.0 == [255, 0, 0, 255]
	}

	#[test]
	fn identical_images_report_zero() {
		let d = diff(&img(10, 10), &img(10, 10)).unwrap();
		assert_eq!(d.differing, 0);
		assert_eq!(d.bbox, None);
		assert_eq!(d.percent(), 0.0);
		// Even the strictest gate passes on identical images.
		assert!(!d.exceeds(0.0));
	}

	#[test]
	fn one_changed_pixel_sets_count_and_bbox() {
		let a = img(10, 10);
		let mut b = img(10, 10);
		b.put_pixel(3, 4, image::Rgba([11, 20, 30, 255])); // one channel
		let d = diff(&a, &b).unwrap();
		assert_eq!(d.differing, 1);
		assert_eq!(d.bbox, Some(BBox { x1: 3, y1: 4, x2: 3, y2: 4 }));
		assert!((d.percent() - 1.0).abs() < 1e-9, "{}", d.percent());
	}

	#[test]
	fn bbox_spans_every_changed_pixel() {
		let a = img(20, 20);
		let mut b = img(20, 20);
		for (x, y) in [(5, 7), (15, 7), (5, 18), (11, 12)] {
			b.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
		}
		let d = diff(&a, &b).unwrap();
		assert_eq!(d.differing, 4);
		assert_eq!(d.bbox, Some(BBox { x1: 5, y1: 7, x2: 15, y2: 18 }));
	}

	#[test]
	fn size_mismatch_is_an_error() {
		let err = diff(&img(10, 10), &img(10, 11)).unwrap_err();
		assert!(err.to_string().contains("size mismatch"), "{err}");
	}

	#[test]
	fn alpha_only_change_is_not_a_difference() {
		let a = img(4, 4);
		let mut b = img(4, 4);
		b.put_pixel(1, 1, image::Rgba([10, 20, 30, 0])); // transparent, same RGB
		assert_eq!(diff(&a, &b).unwrap().differing, 0);
	}

	#[test]
	fn region_parses_and_rejects_junk() {
		assert_eq!(parse_region("0,0,10,10").unwrap(), [0, 0, 10, 10]);
		assert_eq!(parse_region(" 1 , 2 , 3 , 4 ").unwrap(), [1, 2, 3, 4]);
		assert!(parse_region("0,0,10").is_err());
		assert!(parse_region("a,b,c,d").is_err());
		assert!(parse_region("").is_err());
	}

	#[test]
	fn crop_clamps_to_the_image() {
		let full = crop(&img(100, 50), [-5, -5, 999, 999]).unwrap();
		assert_eq!(full.dimensions(), (100, 50));
		let part = crop(&img(100, 50), [10, 20, 12, 22]).unwrap();
		assert_eq!(part.dimensions(), (2, 2));
	}

	#[test]
	fn crop_rejects_an_empty_region() {
		let err = crop(&img(10, 10), [5, 5, 5, 10]).unwrap_err();
		assert!(err.to_string().contains("empty region"), "{err}");
	}

	#[test]
	fn percent_is_of_the_compared_area() {
		// A change of 2 pixels inside a 10x10 crop of a 20x20 image: the
		// crop area is what counts, not the full image.
		let a = img(20, 20);
		let mut b = img(20, 20);
		b.put_pixel(1, 1, image::Rgba([0, 0, 0, 255]));
		b.put_pixel(8, 8, image::Rgba([0, 0, 0, 255]));
		let a = crop(&a, [0, 0, 10, 10]).unwrap();
		let b = crop(&b, [0, 0, 10, 10]).unwrap();
		let d = diff(&a, &b).unwrap();
		assert_eq!(d.compared(), 100);
		assert_eq!(d.differing, 2);
		assert!((d.percent() - 2.0).abs() < 1e-9);
	}

	#[test]
	fn exceeds_is_strictly_above_the_threshold() {
		let d = Diff { width: 10, height: 10, differing: 1, bbox: None };
		assert!((d.percent() - 1.0).abs() < 1e-9);
		assert!(!d.exceeds(1.0), "exactly at the threshold passes");
		assert!(d.exceeds(0.5));
		assert!(d.exceeds(0.0), "any change fails the default gate");
	}

	#[test]
	fn overlay_marks_changed_pixels_and_frames_the_bbox() {
		let a = img(20, 20);
		let mut b = img(20, 20);
		b.put_pixel(5, 7, image::Rgba([0, 0, 0, 255]));
		b.put_pixel(15, 18, image::Rgba([0, 0, 0, 255]));
		let d = diff(&a, &b).unwrap();
		let o = overlay(&a, &b, &d);

		// Every differing pixel is red…
		assert!(is_red(o.get_pixel(5, 7)));
		assert!(is_red(o.get_pixel(15, 18)));
		// …the bbox is framed (2px border)…
		assert!(is_red(o.get_pixel(5, 8)), "left edge");
		assert!(is_red(o.get_pixel(14, 18)), "bottom edge");
		assert!(is_red(o.get_pixel(15, 17)), "right edge");
		assert!(is_red(o.get_pixel(6, 7)), "top edge");
		// …and everything else is left as it was.
		assert_eq!(o.get_pixel(10, 10), a.get_pixel(10, 10));
		assert_eq!(o.get_pixel(0, 0), a.get_pixel(0, 0));
	}

	#[test]
	fn overlay_of_identical_images_is_a_copy() {
		let a = img(8, 8);
		let d = diff(&a, &img(8, 8)).unwrap();
		assert_eq!(overlay(&a, &img(8, 8), &d), a);
	}
}
