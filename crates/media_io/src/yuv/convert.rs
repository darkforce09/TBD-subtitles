//! 8-bit YUV 4:2:0 pictures and their conversion to packed R′G′B′: whole, padded below to a
//! batch's height, or one rectangle of it.
//!
//! **Role:** borrow a decoded frame's planes, planar (yuv420p) or with interleaved chroma (nv12),
//! and write rgb24 rows from them in the stream's own matrix and range, on one thread or one row
//! per rayon task.
//!
//! **Position:** used by the detection scan for the frames it screens, by the region source for
//! crops, and by the detection benchmark.
//!
//! **Signals and state:** none; the caller owns every buffer.
//!
//! **Invariants:** a picture has even, non-zero dimensions and planes of exactly its size, or it
//! is refused; the serial and parallel conversions give identical bytes; padding rows are black;
//! a rectangle lies inside the picture or nothing is written.

use rayon::prelude::*;

use super::colour::Coefficients;

/// The chroma planes of a 4:2:0 picture.
#[derive(Debug, Clone, Copy)]
pub enum Chroma<'a> {
    /// U then V, each a quarter-size plane (yuv420p).
    Planar { u: &'a [u8], v: &'a [u8] },
    /// One half-height plane of U and V pairs (nv12).
    Interleaved(&'a [u8]),
}

/// One borrowed 8-bit YUV 4:2:0 picture.
#[derive(Debug, Clone, Copy)]
pub struct Yuv420<'a> {
    pub width: usize,
    pub height: usize,
    pub luma: &'a [u8],
    pub chroma: Chroma<'a>,
}

/// The byte size of a `width` × `height` 4:2:0 picture, or `None` for odd or zero dimensions.
pub fn picture_bytes(width: usize, height: usize) -> Option<usize> {
    sizes(width, height).map(|(luma, quarter)| luma + 2 * quarter)
}

impl<'a> Yuv420<'a> {
    /// A yuv420p picture of `width` × `height` in `bytes`, or `None` when the sizes disagree.
    pub fn planar(bytes: &'a [u8], width: usize, height: usize) -> Option<Yuv420<'a>> {
        let (luma, quarter) = sizes(width, height)?;
        (bytes.len() == luma + 2 * quarter).then(|| Yuv420 {
            width,
            height,
            luma: &bytes[..luma],
            chroma: Chroma::Planar {
                u: &bytes[luma..luma + quarter],
                v: &bytes[luma + quarter..],
            },
        })
    }

    /// An nv12 picture of `width` × `height` in `bytes`, or `None` when the sizes disagree.
    pub fn nv12(bytes: &'a [u8], width: usize, height: usize) -> Option<Yuv420<'a>> {
        let (luma, quarter) = sizes(width, height)?;
        (bytes.len() == luma + 2 * quarter).then(|| Yuv420 {
            width,
            height,
            luma: &bytes[..luma],
            chroma: Chroma::Interleaved(&bytes[luma..]),
        })
    }

    /// The byte size of the picture as rgb24.
    pub fn rgb_len(&self) -> usize {
        self.width * self.height * 3
    }

    /// Picture row `row`'s luma samples.
    pub fn luma_row(&self, row: usize) -> &'a [u8] {
        &self.luma[row * self.width..(row + 1) * self.width]
    }
}

/// The luma and one chroma plane's byte sizes, for even non-zero dimensions.
fn sizes(width: usize, height: usize) -> Option<(usize, usize)> {
    let even = width > 0 && height > 0 && width.is_multiple_of(2) && height.is_multiple_of(2);
    even.then(|| (width * height, width * height / 4))
}

/// A rectangle of picture pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    /// Whether the rectangle is non-empty and inside `picture`.
    pub fn inside(&self, picture: &Yuv420<'_>) -> bool {
        self.width > 0
            && self.height > 0
            && self
                .x
                .checked_add(self.width)
                .is_some_and(|r| r <= picture.width)
            && self
                .y
                .checked_add(self.height)
                .is_some_and(|b| b <= picture.height)
    }
}

/// Convert `picture` into `rgb` on this thread; `false` when `rgb` is not the picture's size.
pub fn to_rgb(picture: &Yuv420<'_>, colour: &Coefficients, rgb: &mut [u8]) -> bool {
    if rgb.len() != picture.rgb_len() {
        return false;
    }
    for (row, out) in rgb.chunks_exact_mut(picture.width * 3).enumerate() {
        convert_row(picture, colour, row, 0, out);
    }
    true
}

/// Convert `picture` into `rgb`, one row per rayon task; `false` when `rgb` is not the picture's
/// size.
pub fn to_rgb_parallel(picture: &Yuv420<'_>, colour: &Coefficients, rgb: &mut [u8]) -> bool {
    if rgb.len() != picture.rgb_len() {
        return false;
    }
    rgb.par_chunks_exact_mut(picture.width * 3)
        .enumerate()
        .for_each(|(row, out)| convert_row(picture, colour, row, 0, out));
    true
}

/// Convert `picture` into the top of `rgb`, a reused `width` × `padded_height` rgb24 buffer, one
/// row per rayon task, and paint the rows below the picture black; `false` when `rgb` is not that
/// size or `padded_height` is shorter than the picture.
pub fn to_rgb_padded_into(
    picture: &Yuv420<'_>,
    colour: &Coefficients,
    padded_height: usize,
    rgb: &mut [u8],
) -> bool {
    if padded_height < picture.height || rgb.len() != picture.width * padded_height * 3 {
        return false;
    }
    rgb.par_chunks_exact_mut(picture.width * 3)
        .enumerate()
        .for_each(|(row, out)| {
            if row < picture.height {
                convert_row(picture, colour, row, 0, out);
            } else {
                out.fill(0);
            }
        });
    true
}

/// The rgb24 pixels of `rect` of `picture` into `rgb`, resized to the rectangle, one row per
/// rayon task; `false`, writing nothing, when the rectangle is not inside the picture.
pub fn crop_to_rgb(
    picture: &Yuv420<'_>,
    colour: &Coefficients,
    rect: Rect,
    rgb: &mut Vec<u8>,
) -> bool {
    if !rect.inside(picture) {
        return false;
    }
    rgb.resize(rect.width * rect.height * 3, 0);
    rgb.par_chunks_exact_mut(rect.width * 3)
        .enumerate()
        .for_each(|(row, out)| convert_row(picture, colour, rect.y + row, rect.x, out));
    true
}

/// Convert `out.len() / 3` pixels of picture row `row`, starting at column `x`, into `out`.
fn convert_row(picture: &Yuv420<'_>, colour: &Coefficients, row: usize, x: usize, out: &mut [u8]) {
    let width = picture.width;
    let luma = &picture.luma[row * width..(row + 1) * width];
    let half = width / 2;
    let chroma_row = row / 2;
    match picture.chroma {
        Chroma::Planar { u, v } => {
            let u = &u[chroma_row * half..(chroma_row + 1) * half];
            let v = &v[chroma_row * half..(chroma_row + 1) * half];
            for (i, pixel) in out.chunks_exact_mut(3).enumerate() {
                let column = x + i;
                pixel.copy_from_slice(&colour.rgb(luma[column], u[column / 2], v[column / 2]));
            }
        }
        Chroma::Interleaved(uv) => {
            let uv = &uv[chroma_row * width..(chroma_row + 1) * width];
            for (i, pixel) in out.chunks_exact_mut(3).enumerate() {
                let column = x + i;
                let pair = (column / 2) * 2;
                pixel.copy_from_slice(&colour.rgb(luma[column], uv[pair], uv[pair + 1]));
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/convert.rs"]
mod tests;
