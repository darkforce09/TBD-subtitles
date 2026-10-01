//! YUV 4:2:0 to RGB conversion in Rust, for timing against FFmpeg's own conversion.
//!
//! **Role:** turn one 8-bit YUV 4:2:0 frame, planar (yuv420p) or with interleaved chroma (nv12),
//! into packed rgb24 with BT.709 limited-range integer arithmetic, row by row on one thread or
//! across rayon's threads.
//! **Position:** the colour conversion rows of `detect-bench`'s decode section.
//! **Signals and state:** none; the caller owns both buffers.
//! **Invariants:** both versions give identical bytes; a frame has even dimensions and buffers of
//! exactly its size, or it is refused.

use rayon::prelude::*;

/// The chroma planes of a 4:2:0 frame.
#[derive(Clone, Copy)]
pub enum Chroma<'a> {
    /// U then V, each a quarter-size plane (yuv420p).
    Planar { u: &'a [u8], v: &'a [u8] },
    /// One half-height plane of U and V pairs (nv12).
    Interleaved(&'a [u8]),
}

/// One borrowed 8-bit YUV 4:2:0 frame.
#[derive(Clone, Copy)]
pub struct Yuv420<'a> {
    pub width: usize,
    pub height: usize,
    pub luma: &'a [u8],
    pub chroma: Chroma<'a>,
}

impl<'a> Yuv420<'a> {
    /// A yuv420p frame of `width` × `height` in `bytes`, or `None` when the sizes disagree.
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

    /// An nv12 frame of `width` × `height` in `bytes`, or `None` when the sizes disagree.
    pub fn nv12(bytes: &'a [u8], width: usize, height: usize) -> Option<Yuv420<'a>> {
        let (luma, quarter) = sizes(width, height)?;
        (bytes.len() == luma + 2 * quarter).then(|| Yuv420 {
            width,
            height,
            luma: &bytes[..luma],
            chroma: Chroma::Interleaved(&bytes[luma..]),
        })
    }

    /// The byte size of the frame's rgb24 picture.
    pub fn rgb_len(&self) -> usize {
        self.width * self.height * 3
    }
}

/// The luma and one chroma plane's byte sizes, for even non-zero dimensions.
fn sizes(width: usize, height: usize) -> Option<(usize, usize)> {
    let even = width > 0 && height > 0 && width.is_multiple_of(2) && height.is_multiple_of(2);
    even.then(|| (width * height, width * height / 4))
}

/// Convert `frame` into `rgb` on this thread; `false` when `rgb` is not the frame's size.
pub fn to_rgb(frame: &Yuv420<'_>, rgb: &mut [u8]) -> bool {
    if rgb.len() != frame.rgb_len() {
        return false;
    }
    for (row, out) in rgb.chunks_exact_mut(frame.width * 3).enumerate() {
        convert_row(frame, row, out);
    }
    true
}

/// Convert `frame` into `rgb`, one row per rayon task; `false` when `rgb` is not the frame's size.
pub fn to_rgb_parallel(frame: &Yuv420<'_>, rgb: &mut [u8]) -> bool {
    if rgb.len() != frame.rgb_len() {
        return false;
    }
    rgb.par_chunks_exact_mut(frame.width * 3)
        .enumerate()
        .for_each(|(row, out)| convert_row(frame, row, out));
    true
}

/// Convert picture row `row` of `frame` into `out`, its packed rgb24 row.
fn convert_row(frame: &Yuv420<'_>, row: usize, out: &mut [u8]) {
    let width = frame.width;
    let luma = &frame.luma[row * width..(row + 1) * width];
    let half = width / 2;
    let chroma_row = row / 2;
    match frame.chroma {
        Chroma::Planar { u, v } => {
            let u = &u[chroma_row * half..(chroma_row + 1) * half];
            let v = &v[chroma_row * half..(chroma_row + 1) * half];
            for (x, pixel) in out.chunks_exact_mut(3).enumerate() {
                pixel.copy_from_slice(&bt709(luma[x], u[x / 2], v[x / 2]));
            }
        }
        Chroma::Interleaved(uv) => {
            let uv = &uv[chroma_row * width..(chroma_row + 1) * width];
            for (x, pixel) in out.chunks_exact_mut(3).enumerate() {
                let pair = (x / 2) * 2;
                pixel.copy_from_slice(&bt709(luma[x], uv[pair], uv[pair + 1]));
            }
        }
    }
}

/// One BT.709 limited-range pixel in 13-bit fixed point, rounded and clamped to 0–255.
pub fn bt709(y: u8, u: u8, v: u8) -> [u8; 3] {
    // 1.164383, 1.792741, 0.213249, 0.532909 and 2.112402, times 8192.
    const LUMA: i32 = 9539;
    const RED_V: i32 = 14686;
    const GREEN_U: i32 = 1747;
    const GREEN_V: i32 = 4366;
    const BLUE_U: i32 = 17305;
    const ROUND: i32 = 1 << 12;
    let y = (i32::from(y) - 16) * LUMA;
    let u = i32::from(u) - 128;
    let v = i32::from(v) - 128;
    let channel = |value: i32| ((value + ROUND) >> 13).clamp(0, 255) as u8;
    [
        channel(y + RED_V * v),
        channel(y - GREEN_U * u - GREEN_V * v),
        channel(y + BLUE_U * u),
    ]
}

#[cfg(test)]
#[path = "tests/yuv.rs"]
mod tests;
