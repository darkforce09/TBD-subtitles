//! Diagnostics of stroke segmentation for one occurrence.
//!
//! **Role:** report every figure the stroke-mask step judges an occurrence's keyframe by: its
//! rectangles, each colour partition's coverage, cut share and largest piece with the guard it
//! failed, the completeness counts, the refitted lettering area and the verdict.
//! **Position:** the public diagnostic face of mask extraction for repository tooling; runs the
//! same keyframe location and segmentation as [`super::extract`], without collecting plates.
//! **Signals and state:** decodes the keyframe plate through a [`RegionSource`]; writes nothing.
//! **Invariants:** figures come from the production code path, never from a copy of it; the
//! verdict matches what extraction decides at the keyframe.

use image::{GrayImage, RgbImage};
use job_model::onscreen::{LetteringStyle, PixelRect, Quad, TextOccurrence};

use super::{RegionSource, decode_one, follow, locate, select};
use crate::onscreen_text::TextResult;

/// How segmentation read one colour partition of the analysis window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    /// Lettering drawn over the background the window's border ring shows.
    Lettering,
    /// Outlined lettering over the ring's background that fills a box drawn tight around its
    /// outline, read when plain lettering fails or finds no outline: a fill and outline wrapping
    /// each other, separated by at least 2.0 spreads, covering more of the quad than plain
    /// lettering may.
    DenseOutlined,
    /// Writing printed on a panel or sign filling the box.
    Panel,
    /// Outlined lettering whose fill and outline colours also run along the box's edges.
    OutlinedOverRing,
}

/// One colour partition as segmentation judged it.
#[derive(Debug, Clone, PartialEq)]
pub struct Judgement {
    pub reading: Reading,
    /// Clusters in the partition.
    pub colours: usize,
    /// The weakest core separation in spreads; `None` when no colour reads as ink.
    pub separation: Option<f32>,
    /// Share of the line's quad the ink covers.
    pub coverage: f64,
    /// Share of the ink inside the quad and furigana in pieces the window cuts and nothing
    /// follows.
    pub cut_share: f64,
    /// Longest side of the largest 8-connected ink piece, in pixels.
    pub largest_piece: u32,
    /// The first guard the partition failed, or `None` when it passed.
    pub failure: Option<&'static str>,
    /// Whether segmentation chose this partition.
    pub chosen: bool,
}

/// How much ink-coloured picture joined the chosen ink and how much stayed apart from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Completeness {
    /// Pixels joined to the mask: ink-coloured and 8-connected to it near the window.
    pub joined: usize,
    /// Ink-coloured pixels inside the quad and furigana that the erase mask leaves.
    pub stray: usize,
    /// Pixels of the dilated erase mask.
    pub mask_area: usize,
}

/// The best match of the keyframe writing in one frame of a moving occurrence's span.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Followed {
    pub frame: u64,
    /// Shift of the writing from the keyframe, in pixels, and its scale; `None` when no candidate
    /// position fits the frame.
    pub placement: Option<(i32, i32, f64)>,
    /// Zero-mean normalized cross-correlation of the match.
    pub score: f32,
    /// Whether the match is close enough to follow the writing.
    pub followed: bool,
}

/// Every frame's match of moving writing and what they make of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Following {
    pub frames: Vec<Followed>,
    /// `Ok(true)` when the writing counts as still, `Ok(false)` when it is followed as it moves,
    /// else why it cannot be followed.
    pub path: Result<bool, &'static str>,
}

/// Everything segmentation measured on the way to a verdict.
#[derive(Debug, Clone, Default)]
pub struct Trace {
    pub partitions: Vec<Judgement>,
    pub completeness: Option<Completeness>,
}

/// One occurrence's segmentation figures at its keyframe.
#[derive(Debug, Clone)]
pub struct Diagnosis {
    /// The line's quad at the keyframe.
    pub quad: Quad,
    /// The line's quad plus the analysis margin.
    pub window: PixelRect,
    /// The pixels segmentation examined first; a loose box that did not separate there was tried
    /// again within `window`, and its partitions follow the first attempt's in the trace.
    pub analysis: PixelRect,
    /// The keyframe plate.
    pub plate: PixelRect,
    pub line_height: f64,
    /// The frame segmented.
    pub keyframe: u64,
    pub trace: Trace,
    /// The lettering area refitted to the ink, for a loose Claude box that separated.
    pub lettering: Option<Quad>,
    pub style: Option<LetteringStyle>,
    /// The keyframe plate's pixels.
    pub pixels: RgbImage,
    /// The erase mask over the plate, when the strokes separate.
    pub mask: Option<GrayImage>,
    /// `Ok` when the strokes separate, else the fallback reason.
    pub verdict: Result<(), &'static str>,
    /// How moving writing was followed through its span: `None` for static writing or writing
    /// that does not separate, else every frame's best match or why it cannot be followed.
    pub following: Option<Result<Following, &'static str>>,
}

/// Segment `occurrence` at its keyframe as extraction does and report every figure; an
/// occurrence without frames or position reports only the fallback reason as an error.
pub fn diagnose(
    occurrence: &TextOccurrence,
    source: &mut dyn RegionSource,
) -> TextResult<Result<Diagnosis, &'static str>> {
    let Some(span) = select::span(source.timeline(), occurrence.start_s, occurrence.end_s) else {
        return Ok(Err(super::NO_FRAMES));
    };
    let (keyframe, areas) = match locate(occurrence, source, span) {
        Ok(found) => found,
        Err(reason) => return Ok(Err(reason)),
    };
    let key_crop = decode_one(source, areas.plate, keyframe)?;
    let mut trace = Trace::default();
    let outcome = super::separate(&key_crop, &areas, &mut trace);
    let following = match &outcome {
        Ok(_) => match super::tracker(occurrence, &key_crop, &areas) {
            None => None,
            Some(Err(reason)) => Some(Err(reason)),
            Some(Ok(tracker)) => Some(followed(source, &tracker, span)?),
        },
        Err(_) => None,
    };
    let (lettering, style, mask, verdict) = match outcome {
        Ok(segmentation) => (
            segmentation.lettering,
            Some(segmentation.style),
            Some(segmentation.mask),
            Ok(()),
        ),
        Err(reason) => (None, None, None, Err(reason)),
    };
    Ok(Ok(Diagnosis {
        quad: areas.quad,
        window: areas.window,
        analysis: areas.analysis,
        plate: areas.plate,
        line_height: areas.line_height,
        keyframe,
        trace,
        lettering,
        style,
        pixels: key_crop,
        mask,
        verdict,
        following,
    }))
}

/// Every frame's best match of the writing `tracker` follows through `span`.
fn followed(
    source: &mut dyn RegionSource,
    tracker: &follow::Tracker,
    span: (u64, u64),
) -> TextResult<Result<Following, &'static str>> {
    let found = match follow::locate_span(source, tracker, span)? {
        Ok(found) => found,
        Err(reason) => return Ok(Err(reason)),
    };
    let path = match follow::path(&found) {
        Ok(follow::Path::Still) => Ok(true),
        Ok(follow::Path::Moving(_)) => Ok(false),
        Err(reason) => Err(reason),
    };
    let frames = (span.0..)
        .zip(found)
        .map(|(frame, best)| Followed {
            frame,
            placement: best.map(|(p, _)| (p.dx, p.dy, p.factor())),
            score: best.map_or(f32::NEG_INFINITY, |(_, s)| s),
            followed: best.is_some_and(|(_, s)| s >= follow::MIN_SCORE),
        })
        .collect();
    Ok(Ok(Following { frames, path }))
}
