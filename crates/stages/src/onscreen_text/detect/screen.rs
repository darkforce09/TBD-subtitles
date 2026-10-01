//! Which frames the scan screens, which of them repeat the last screened picture, and the padded
//! picture a screened frame is sent as.
//!
//! **Role:** choose the sample frames, recognise a sample whose brightness repeats the last
//! screened sample's, and convert a frame to the padded rgb24 picture the detector sessions take.
//! **Position:** helpers of the scan coordinator and of the bisection probes; no decoding, no
//! model and no file output.
//! **Signals and state:** the sample schedule and the last screened sample's luma thumbnail.
//! **Invariants:** every `step`-th frame, both frames around every shot cut and the final frame
//! are samples, so a gap never crosses a cut and holds at most `step - 1` frames; a sample is a
//! repeat only when every 32 by 32 block of its luma stays within a mean of 4 levels of the last
//! screened sample's, so a slow change never drifts through a chain of repeats; padding rows are
//! black and the picture is never stretched.

use inference::ocr::pool::PaddedFrame;
use job_model::outputs::ShotChanges;
use media_io::yuv::{Coefficients, LumaThumbnail, Yuv420, luma_thumbnail, to_rgb_padded_into};

/// The side of a thumbnail cell, in pixels.
const THUMBNAIL_CELL: usize = 8;
/// Thumbnail cells per side of a compared block: 32-pixel blocks.
const BLOCK_CELLS: usize = 4;
/// The mean luma difference of a block above which it has changed.
const BLOCK_MEAN: u32 = 4;

/// Frames between coarse samples: about half a second, at least one.
pub(super) fn sample_step(fps: f64) -> u64 {
    (fps / 2.0).round().max(1.0) as u64
}

/// Which frame indices the detector screens.
pub(super) struct Samples {
    step: u64,
    count: u64,
    /// The first frame at or after every cut and the frame before it, ascending.
    boundaries: Vec<u64>,
}

impl Samples {
    pub(super) fn new(timeline: &[(f64, f64)], cuts: &ShotChanges, step: u64) -> Self {
        let mut boundaries = Vec::with_capacity(cuts.cuts.len() * 2);
        for cut in &cuts.cuts {
            let first = timeline.partition_point(|&(time_s, _)| time_s < cut.time_s) as u64;
            if first < timeline.len() as u64 {
                boundaries.push(first);
            }
            if first > 0 {
                boundaries.push(first - 1);
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        Self {
            step: step.max(1),
            count: timeline.len() as u64,
            boundaries,
        }
    }

    pub(super) fn contains(&self, index: u64) -> bool {
        index.is_multiple_of(self.step)
            || index + 1 == self.count
            || self.boundaries.binary_search(&index).is_ok()
    }
}

/// The last screened sample's brightness, against which later samples count as repeats.
#[derive(Default)]
pub(super) struct Repeats {
    last: Option<LumaThumbnail>,
}

impl Repeats {
    /// Whether `picture` must be screened: `false` when it repeats the last screened sample,
    /// `true` otherwise, and it then becomes the last screened sample.
    pub(super) fn needs_screen(&mut self, picture: &Yuv420<'_>) -> bool {
        let thumbnail = luma_thumbnail(picture, THUMBNAIL_CELL);
        if self
            .last
            .as_ref()
            .is_some_and(|last| thumbnail.matches(last, BLOCK_CELLS, BLOCK_MEAN))
        {
            return false;
        }
        self.last = Some(thumbnail);
        true
    }
}

/// `picture` as rgb24 in the stream's colour, padded below with black rows to a multiple of 32.
pub(super) fn padded(picture: &Yuv420<'_>, colour: &Coefficients) -> PaddedFrame {
    let height = picture.height as u32;
    let padded_height = PaddedFrame::padded(height);
    let mut rgb = vec![0; picture.width * padded_height as usize * 3];
    let converted = to_rgb_padded_into(picture, colour, padded_height as usize, &mut rgb);
    debug_assert!(converted, "the buffer is sized for the padded picture");
    PaddedFrame {
        width: picture.width as u32,
        height,
        padded_height,
        rgb,
    }
}

#[cfg(test)]
#[path = "tests/screen.rs"]
mod tests;
