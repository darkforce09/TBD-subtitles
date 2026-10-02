//! Coarse-to-fine detection of visible writing on the full-resolution video.
//!
//! **Role:** screen half-second samples of every frame decoded at full resolution, follow text
//! regions between samples, narrow each region's first and last frame exactly by bisecting the
//! frames between samples, then confirm every occurrence once on its keyframe.
//! **Position:** first visual stage; reads a `FrameSource` and screens through a `TextScreening`
//! pool of detector sessions, and writes crops and keyframe stills under the job's `visual/`
//! folder.
//! **Signals and state:** the frames of the groups waiting for their screening results, the
//! active regions with fixed anchor boxes, the keyframe candidates within their budget, and the
//! PNG writer thread during confirmation.
//! **Invariants:** document quads are in source pixels; every comparison of a region crops the
//! current frame's luma at the region's anchor box, so detector jitter never splits static
//! writing; an occurrence's frames tile from its entry frame to its first absent frame; a gap
//! never crosses a shot cut; results are applied in sample order whatever order they arrive in;
//! writing shorter than `MIN_OCCURRENCE_S`, wider than `MAX_REGION_SHARE` of the frame or
//! unconfirmed on its keyframe is dropped as screening noise; the limits fail explicitly instead
//! of dropping text; no full-video image extraction occurs.

mod confirm;
mod crops;
mod probe;
mod regions;
mod scan;
mod screen;
mod source;
mod timing;
mod window;
mod writer;

use std::path::Path;

use inference::ocr::pool::TextScreening;
use job_model::onscreen::TextDocument;
use job_model::outputs::{ShotChanges, VideoStream};

use super::TextResult;

pub use crops::crop;
pub use source::{FfmpegSource, FrameSource};
pub use timing::ScanStats;

/// The default minimum frames for an occurrence to qualify its keyframe for server confirmation.
/// Occurrences on screen for >= 2.0s (>= 5 sampled/bisected frames) represent persistent
/// signs and title cards. Shorter co-occurring text sharing these keyframes is confirmed for free.
pub(crate) const MIN_CONFIRM_FRAMES: usize = 5;

/// The default minimum confidence for an occurrence to qualify its keyframe for server confirmation.
/// Real on-screen signs in animation score >= 0.60, while persistent background noise scores
/// below 0.55.
pub(crate) const MIN_CONFIRM_CONFIDENCE: f64 = 0.55;

/// Minimum consecutive screening samples for an isolated candidate to qualify for bisection.
/// Standalone 2-sample flickers (< 1.0s) are transient noise; real signs persist >= 1.33s.
pub(crate) const MIN_BISECTION_SAMPLES: usize = 3;

/// The bounds a scan runs within.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ScanLimits {
    /// The bytes the keyframe candidates may hold in memory.
    pub(crate) candidate_budget: usize,
    /// Minimum frames for an occurrence to qualify its keyframe for server confirmation.
    pub(crate) min_confirm_frames: usize,
    /// Minimum confidence for an occurrence to qualify its keyframe for server confirmation.
    pub(crate) min_confirm_confidence: f64,
    /// Minimum samples for an occurrence to qualify for entry and exit bisection.
    pub(crate) min_bisection_samples: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            candidate_budget: window::CANDIDATE_BUDGET,
            min_confirm_frames: MIN_CONFIRM_FRAMES,
            min_confirm_confidence: MIN_CONFIRM_CONFIDENCE,
            min_bisection_samples: MIN_BISECTION_SAMPLES,
        }
    }
}

/// Finds every text occurrence of the video with exact frame boundaries, one crop and one
/// keyframe still each.
pub fn scan(
    source: &mut dyn FrameSource,
    stream: &VideoStream,
    cuts: &ShotChanges,
    root: &Path,
    pool: &mut dyn TextScreening,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<TextDocument> {
    scan_measured(source, stream, cuts, root, pool, progress).map(|(document, _)| document)
}

/// `scan`, with where its time went, what it handled and what the detector sessions reported.
pub fn scan_measured(
    source: &mut dyn FrameSource,
    stream: &VideoStream,
    cuts: &ShotChanges,
    root: &Path,
    pool: &mut dyn TextScreening,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<(TextDocument, ScanStats)> {
    scan::run(
        source,
        stream,
        cuts,
        root,
        pool,
        progress,
        ScanLimits::default(),
    )
}

/// The stream's frame rate, or 24 when it reports none.
fn frame_rate(stream: &VideoStream) -> f64 {
    stream
        .fps()
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .unwrap_or(24.0)
}

#[cfg(test)]
#[path = "tests/fixtures.rs"]
mod fixtures;
