//! One plate through a square inpainting model: fitting the rectangle to the model's side,
//! overlapping tiles, and the feathered blend back onto the source pixels.
//!
//! **Role:** turn a source crop and its erase mask into the crop with the masked pixels filled.
//! **Position:** called by the inpainting stage in `mod.rs` for each plate it does not already
//! hold; calls the `Inpaint` model once per square tile that contains a masked pixel.
//! **Signals and state:** a working copy at model scale (the crop itself, the crop scaled so its
//! longest side is the model side, or the crop at half size) and one weighted accumulator of the
//! tiles' results.
//! **Invariants:** pixels further than one pixel from the mask are the source bytes; every masked
//! pixel takes the filled value; a scaled mask never loses a stroke; tiles overlap by at least a
//! quarter of the side and blend linearly across the overlap.

use image::imageops::{self, FilterType};
use image::{GrayImage, RgbImage};

use super::Inpaint;
use crate::onscreen_text::TextResult;

/// The smallest model side the tiling supports.
const MIN_SIDE: usize = 8;

/// The source crop with its masked pixels filled by `model`.
pub(super) fn fill_plate(
    source: &RgbImage,
    mask: &GrayImage,
    model: &mut dyn Inpaint,
) -> TextResult<RgbImage> {
    let side = model.side();
    if side < MIN_SIDE {
        return Err(format!("an inpainting side of {side} pixels is too small").into());
    }
    if source.dimensions() != mask.dimensions() {
        return Err("a plate's source and mask differ in size".into());
    }
    let (width, height) = source.dimensions();
    let working = working_size(width, height, side);
    let filled = if working == (width, height) {
        fill_tiles(source, mask, model)?
    } else {
        let small_source = imageops::resize(source, working.0, working.1, FilterType::Triangle);
        let small_mask = dilate(&max_pool(mask, working.0, working.1));
        let small = fill_tiles(&small_source, &small_mask, model)?;
        imageops::resize(&small, width, height, FilterType::Triangle)
    };
    Ok(feathered_blend(source, &filled, mask))
}

/// The size the model works at: the crop when it fits the side, the crop scaled so its longest
/// side is the model side when that is at most half as large, and half size otherwise.
pub(super) fn working_size(width: u32, height: u32, side: usize) -> (u32, u32) {
    let side = side as u64;
    let longest = u64::from(width.max(height));
    if longest <= side {
        return (width, height);
    }
    if longest <= 2 * side {
        let scale = |v: u32| ((u64::from(v) * side + longest / 2) / longest).max(1) as u32;
        return (scale(width), scale(height));
    }
    (width.div_ceil(2), height.div_ceil(2))
}

/// Tile origins along one axis of `len` pixels: one tile when it fits, otherwise evenly spread
/// tiles from both ends whose neighbours overlap by at least `side / 4`.
pub(super) fn tile_positions(len: usize, side: usize) -> Vec<usize> {
    if len <= side {
        return vec![0];
    }
    let stride = side - side / 4;
    let span = len - side;
    let count = span.div_ceil(stride) + 1;
    (0..count)
        .map(|i| (i * span + (count - 1) / 2) / (count - 1))
        .collect()
}

/// Fill the masked pixels of `image` tile by tile at the model side; tiles without a masked
/// pixel keep their pixels and cost no model call.
fn fill_tiles(image: &RgbImage, mask: &GrayImage, model: &mut dyn Inpaint) -> TextResult<RgbImage> {
    let side = model.side();
    let (width, height) = (image.width() as usize, image.height() as usize);
    let columns = tile_positions(width, side);
    let rows = tile_positions(height, side);
    let mut sum = vec![0.0_f32; width * height * 3];
    let mut weight = vec![0.0_f32; width * height];
    for (row, &y0) in rows.iter().enumerate() {
        let ramp_y = Ramp::new(&rows, row, side);
        for (column, &x0) in columns.iter().enumerate() {
            let ramp_x = Ramp::new(&columns, column, side);
            let (rgb, erase) = canvas(image, mask, x0, y0, side);
            let result = if erase.iter().any(|&m| m > 0) {
                let result = model.inpaint(&rgb, &erase)?;
                if result.len() != rgb.len() {
                    return Err(format!(
                        "the inpainting model returned {} bytes for a {side} × {side} tile",
                        result.len()
                    )
                    .into());
                }
                result
            } else {
                rgb
            };
            for ly in 0..side.min(height - y0) {
                let wy = ramp_y.at(ly);
                for lx in 0..side.min(width - x0) {
                    let w = wy * ramp_x.at(lx);
                    let pixel = (y0 + ly) * width + x0 + lx;
                    weight[pixel] += w;
                    for c in 0..3 {
                        sum[pixel * 3 + c] += w * f32::from(result[(ly * side + lx) * 3 + c]);
                    }
                }
            }
        }
    }
    let pixels = sum
        .iter()
        .enumerate()
        .map(|(i, &s)| (s / weight[i / 3]).round().clamp(0.0, 255.0) as u8)
        .collect();
    RgbImage::from_raw(image.width(), image.height(), pixels)
        .ok_or_else(|| "the filled tiles do not form a picture".into())
}

/// A tile's blend weight along one axis: rising across the overlap with the previous tile and
/// falling across the overlap with the next, so neighbouring weights sum to one.
struct Ramp {
    before: Option<f32>,
    after: Option<f32>,
    side: usize,
}

impl Ramp {
    fn new(positions: &[usize], index: usize, side: usize) -> Ramp {
        let at = positions[index];
        let overlap = |a: usize, b: usize| (a + side).saturating_sub(b).max(1) as f32;
        Ramp {
            before: index
                .checked_sub(1)
                .map(|previous| overlap(positions[previous], at)),
            after: positions.get(index + 1).map(|&next| overlap(at, next)),
            side,
        }
    }

    fn at(&self, local: usize) -> f32 {
        let rise = self
            .before
            .map_or(1.0, |overlap| ((local as f32 + 0.5) / overlap).min(1.0));
        let fall = self.after.map_or(1.0, |overlap| {
            (((self.side - local) as f32 - 0.5) / overlap).min(1.0)
        });
        rise * fall
    }
}

/// The `side` × `side` model input whose top-left is (`x0`, `y0`): pixels past the picture's
/// edge mirror the picture, and so does the mask, so mirrored strokes are erased rather than
/// offered to the model as context.
pub(super) fn canvas(
    image: &RgbImage,
    mask: &GrayImage,
    x0: usize,
    y0: usize,
    side: usize,
) -> (Vec<u8>, Vec<u8>) {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let mut rgb = Vec::with_capacity(side * side * 3);
    let mut erase = Vec::with_capacity(side * side);
    for ly in 0..side {
        let y = mirror(y0 + ly, height) as u32;
        for lx in 0..side {
            let x = mirror(x0 + lx, width) as u32;
            rgb.extend_from_slice(&image.get_pixel(x, y).0);
            erase.push(mask.get_pixel(x, y).0[0]);
        }
    }
    (rgb, erase)
}

/// `index` reflected into `0..len` (the edge pixel repeats once at each turn).
pub(super) fn mirror(index: usize, len: usize) -> usize {
    let period = 2 * len;
    let folded = index % period;
    if folded < len {
        folded
    } else {
        period - 1 - folded
    }
}

/// `mask` at `width` × `height`: a pixel is set when any source pixel it covers is set.
pub(super) fn max_pool(mask: &GrayImage, width: u32, height: u32) -> GrayImage {
    let (w, h) = (u64::from(mask.width()), u64::from(mask.height()));
    let span = |i: u32, from: u64, to: u32| {
        let (i, to) = (u64::from(i), u64::from(to));
        let start = i * from / to;
        let end = ((i + 1) * from).div_ceil(to).clamp(start + 1, from);
        start as u32..end as u32
    };
    GrayImage::from_fn(width, height, |x, y| {
        let set = span(y, h, height)
            .any(|sy| span(x, w, width).any(|sx| mask.get_pixel(sx, sy).0[0] > 0));
        image::Luma([if set { 255 } else { 0 }])
    })
}

/// `mask` grown by one pixel in all eight directions.
pub(super) fn dilate(mask: &GrayImage) -> GrayImage {
    let (width, height) = mask.dimensions();
    GrayImage::from_fn(width, height, |x, y| {
        let set = neighbours(x, width)
            .any(|nx| neighbours(y, height).any(|ny| mask.get_pixel(nx, ny).0[0] > 0));
        image::Luma([if set { 255 } else { 0 }])
    })
}

/// `index` and its neighbours inside `0..len`.
fn neighbours(index: u32, len: u32) -> impl Iterator<Item = u32> + Clone {
    index.saturating_sub(1)..(index + 2).min(len)
}

/// The source with `filled` blended in: fully on masked pixels, and by the share of masked
/// pixels in the 3 × 3 box around each pixel elsewhere, so the fill fades out over one pixel.
pub(super) fn feathered_blend(source: &RgbImage, filled: &RgbImage, mask: &GrayImage) -> RgbImage {
    let (width, height) = source.dimensions();
    RgbImage::from_fn(width, height, |x, y| {
        let original = *source.get_pixel(x, y);
        let alpha = if mask.get_pixel(x, y).0[0] > 0 {
            1.0
        } else {
            let mut set = 0_u32;
            for ny in [y.saturating_sub(1), y, (y + 1).min(height - 1)] {
                for nx in [x.saturating_sub(1), x, (x + 1).min(width - 1)] {
                    set += u32::from(mask.get_pixel(nx, ny).0[0] > 0);
                }
            }
            set as f32 / 9.0
        };
        if alpha == 0.0 {
            return original;
        }
        let fill = filled.get_pixel(x, y);
        image::Rgb(std::array::from_fn(|c| {
            let value = alpha * f32::from(fill.0[c]) + (1.0 - alpha) * f32::from(original.0[c]);
            value.round().clamp(0.0, 255.0) as u8
        }))
    })
}

#[cfg(test)]
#[path = "tests/fill.rs"]
mod tests;
