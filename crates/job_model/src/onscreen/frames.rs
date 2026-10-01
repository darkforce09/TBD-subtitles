//! Contracts for the per-frame rows of in-place replacement: where the writing sits in each
//! frame of an occurrence and which of its pixels the erase covers there.
//!
//! **Role:** carry one `FrameRecord` per frame of every occurrence the stroke-mask step gave
//! plates, from that step to composition, the read-back check and the localized video, which
//! place each frame's lettering by the frame's own shift; encode and decode the erase mask as
//! run-length rows.
//! **Position:** bottom-layer data; the rows of the `frames` table of `job.redb`, keyed by the
//! occurrence id and the frame number.
//! **Signals and state:** serializable values; no I/O.
//! **Invariants:** a mask's runs lie inside the rectangle of the plate the frame belongs to, in
//! row order and left to right within a row, never overlapping or touching; `plate` indexes the
//! occurrence's plates; the shift and the scale are those of the frame's plate placement, the
//! scale equal to the plate's.

use serde::{Deserialize, Serialize};

use super::Quad;

/// One horizontal run of erased pixels, relative to the top-left of the frame's plate.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct RleRun {
    pub row: u16,
    pub start: u16,
    pub len: u16,
}

/// Where the writing sits in one frame and what of it is erased there.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct FrameRecord {
    /// The keyframe's quad of the writing carried onto this frame by its shift and scale, in
    /// source pixels.
    pub quad: Quad,
    /// Zero-mean normalised correlation of the keyframe writing with this frame at its
    /// placement, from -1 to 1; 0 when the keyframe writing has no contrast to correlate.
    pub follow_score: f32,
    /// Movement of the writing relative to the keyframe, in whole source pixels.
    pub shift: [f64; 2],
    /// Scale of the writing relative to the keyframe.
    pub scale: f64,
    /// The pixels erased in this frame, relative to its plate's rectangle.
    pub mask: Vec<RleRun>,
    /// The index of the frame's plate among its occurrence's plates.
    pub plate: u32,
}

/// The runs of the non-zero pixels of a `width` × `height` mask, row by row; `None` when a side
/// does not fit a run's `u16` or `pixels` is not `width * height` long.
pub fn encode_mask(width: u32, height: u32, pixels: &[u8]) -> Option<Vec<RleRun>> {
    let (w, h) = (u16::try_from(width).ok()?, u16::try_from(height).ok()?);
    if pixels.len() != usize::from(w) * usize::from(h) {
        return None;
    }
    let mut runs = Vec::new();
    for row in 0..h {
        let line = &pixels[usize::from(row) * usize::from(w)..][..usize::from(w)];
        let mut x = 0u16;
        while x < w {
            if line[usize::from(x)] == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < w && line[usize::from(x)] != 0 {
                x += 1;
            }
            runs.push(RleRun {
                row,
                start,
                len: x - start,
            });
        }
    }
    Some(runs)
}

/// The `width` × `height` mask `runs` describe: 255 where erased, 0 elsewhere; runs outside the
/// mask are clipped to it.
pub fn decode_mask(width: u32, height: u32, runs: &[RleRun]) -> Vec<u8> {
    let (w, h) = (width as usize, height as usize);
    let mut pixels = vec![0u8; w * h];
    for run in runs {
        let (row, start) = (usize::from(run.row), usize::from(run.start));
        if row >= h || start >= w {
            continue;
        }
        let end = (start + usize::from(run.len)).min(w);
        pixels[row * w + start..row * w + end].fill(u8::MAX);
    }
    pixels
}

/// How many pixels `runs` erase.
pub fn mask_area(runs: &[RleRun]) -> u64 {
    runs.iter().map(|run| u64::from(run.len)).sum()
}

#[cfg(test)]
#[path = "tests/frames.rs"]
mod tests;
