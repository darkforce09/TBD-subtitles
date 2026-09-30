//! Patches in frame samples, and their alpha blend over a decoded 4:2:0 frame.
//!
//! **Role:** convert a composed RGBA patch once into the frame's planar sample values, and mix
//! it into each frame it covers.
//! **Position:** between `patches`, which loads and caches patches, and the render loop.
//! **Signals and state:** a converted patch holds its rectangle, one luma sample and one alpha
//! per pixel, and one Cb, Cr and coverage value per 2×2 chroma block it touches.
//! **Invariants:** out = α·patch + (1 − α)·frame, rounded to the nearest sample: luma per pixel
//! with α in 1/255 steps, chroma per 2×2 block with the block's summed alpha in 1/1020 steps
//! (pixels of the block outside the patch count as α = 0), so any rectangle position and odd
//! size is exact at the block level. α = 0 leaves the bytes untouched and full coverage writes
//! the patch value. 10-bit samples are little-endian 16-bit words.

use image::RgbaImage;
use job_model::onscreen::PixelRect;
use media_io::video_frames::PixelFormat;

use super::LocalizeResult;
use super::colour::Conversion;

/// Full opacity of one pixel.
const OPAQUE: u32 = 255;
/// Full coverage of one 2×2 chroma block: four opaque pixels.
const BLOCK_OPAQUE: u32 = 4 * OPAQUE;

/// A patch as frame samples: luma and alpha per pixel, chroma and coverage per 2×2 block.
#[derive(Debug, Clone, PartialEq)]
pub struct FramePatch {
    pub rect: PixelRect,
    /// One luma sample per patch pixel, row by row.
    pub luma: Vec<u16>,
    /// One alpha (0–255) per patch pixel, row by row.
    pub alpha: Vec<u8>,
    pub chroma: ChromaBlocks,
}

/// The 2×2 chroma blocks a patch touches, in chroma-plane coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct ChromaBlocks {
    /// First block column and row.
    pub x: u32,
    pub y: u32,
    pub columns: u32,
    pub rows: u32,
    /// The alpha-weighted Cb and Cr of the block's patch pixels.
    pub cb: Vec<u16>,
    pub cr: Vec<u16>,
    /// The sum of the block's four pixel alphas (0–1020), outside pixels counting as zero.
    pub coverage: Vec<u16>,
}

impl FramePatch {
    /// The bytes the converted samples occupy.
    pub fn bytes(&self) -> usize {
        self.luma.len() * 2
            + self.alpha.len()
            + (self.chroma.cb.len() + self.chroma.cr.len() + self.chroma.coverage.len()) * 2
    }
}

/// Convert an RGBA patch covering `rect` into frame samples with `conversion`; the image must be
/// the rectangle's size.
pub fn convert(
    image: &RgbaImage,
    rect: PixelRect,
    conversion: Conversion,
) -> LocalizeResult<FramePatch> {
    if image.dimensions() != (rect.width, rect.height) || rect.width == 0 || rect.height == 0 {
        return Err(format!(
            "a patch is {}x{}, not its plate's {}x{}",
            image.width(),
            image.height(),
            rect.width,
            rect.height
        )
        .into());
    }
    let pixels = (rect.width as usize) * (rect.height as usize);
    let mut luma = Vec::with_capacity(pixels);
    let mut alpha = Vec::with_capacity(pixels);
    let mut colour = Vec::with_capacity(pixels);
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        let yuv = conversion.yuv([r, g, b]);
        luma.push(yuv[0].round() as u16);
        alpha.push(a);
        colour.push([yuv[1], yuv[2]]);
    }
    let x = rect.x / 2;
    let y = rect.y / 2;
    let columns = (rect.right() - 1) / 2 - x + 1;
    let rows = (rect.bottom() - 1) / 2 - y + 1;
    let blocks = (columns as usize) * (rows as usize);
    let mut chroma = ChromaBlocks {
        x,
        y,
        columns,
        rows,
        cb: Vec::with_capacity(blocks),
        cr: Vec::with_capacity(blocks),
        coverage: Vec::with_capacity(blocks),
    };
    for row in y..y + rows {
        for column in x..x + columns {
            let (mut sum, mut cb, mut cr) = (0u32, 0.0, 0.0);
            for (px, py) in
                [(0, 0), (1, 0), (0, 1), (1, 1)].map(|(dx, dy)| (column * 2 + dx, row * 2 + dy))
            {
                if px < rect.x || py < rect.y || px >= rect.right() || py >= rect.bottom() {
                    continue;
                }
                let index = ((py - rect.y) * rect.width + (px - rect.x)) as usize;
                let weight = u32::from(alpha[index]);
                sum += weight;
                cb += f64::from(weight) * colour[index][0];
                cr += f64::from(weight) * colour[index][1];
            }
            let average = |total: f64| {
                if sum == 0 {
                    0
                } else {
                    (total / f64::from(sum)).round() as u16
                }
            };
            chroma.cb.push(average(cb));
            chroma.cr.push(average(cr));
            chroma.coverage.push(sum as u16);
        }
    }
    Ok(FramePatch {
        rect,
        luma,
        alpha,
        chroma,
    })
}

/// Blend `patch` over one `format` frame of `size` in place.
pub fn blend(
    frame: &mut [u8],
    format: PixelFormat,
    size: (u32, u32),
    patch: &FramePatch,
) -> LocalizeResult<()> {
    if format == PixelFormat::Rgb24 {
        return Err("patches blend into 4:2:0 frames only".into());
    }
    let expected = format.frame_bytes(size)?;
    if frame.len() != expected {
        return Err(format!("a frame holds {} bytes, not {expected}", frame.len()).into());
    }
    if !patch.rect.inside(size.0, size.1) {
        return Err("a patch lies outside the frame".into());
    }
    let wide = format.is_high_bit_depth();
    let (width, height) = (size.0 as usize, size.1 as usize);
    let luma_samples = width * height;
    let chroma_width = width / 2;
    let chroma_samples = chroma_width * (height / 2);
    let rect = patch.rect;
    for row in 0..rect.height as usize {
        let frame_row = (rect.y as usize + row) * width + rect.x as usize;
        let patch_row = row * rect.width as usize;
        for column in 0..rect.width as usize {
            let weight = u32::from(patch.alpha[patch_row + column]);
            if weight != 0 {
                let value = patch.luma[patch_row + column];
                mix(frame, wide, frame_row + column, value, weight, OPAQUE);
            }
        }
    }
    let blocks = &patch.chroma;
    for row in 0..blocks.rows as usize {
        let frame_row = (blocks.y as usize + row) * chroma_width + blocks.x as usize;
        let patch_row = row * blocks.columns as usize;
        for column in 0..blocks.columns as usize {
            let at = patch_row + column;
            let weight = u32::from(blocks.coverage[at]);
            if weight != 0 {
                let sample = frame_row + column;
                mix(
                    frame,
                    wide,
                    luma_samples + sample,
                    blocks.cb[at],
                    weight,
                    BLOCK_OPAQUE,
                );
                mix(
                    frame,
                    wide,
                    luma_samples + chroma_samples + sample,
                    blocks.cr[at],
                    weight,
                    BLOCK_OPAQUE,
                );
            }
        }
    }
    Ok(())
}

/// Mix `value` into the sample at `index` with opacity `weight` out of `full`, rounding half up.
fn mix(frame: &mut [u8], wide: bool, index: usize, value: u16, weight: u32, full: u32) {
    let old = if wide {
        u32::from(u16::from_le_bytes([frame[2 * index], frame[2 * index + 1]]))
    } else {
        u32::from(frame[index])
    };
    let new = (weight * u32::from(value) + (full - weight) * old + full / 2) / full;
    if wide {
        let bytes = (new as u16).to_le_bytes();
        frame[2 * index] = bytes[0];
        frame[2 * index + 1] = bytes[1];
    } else {
        frame[index] = new as u8;
    }
}

#[cfg(test)]
#[path = "tests/blend.rs"]
mod tests;
