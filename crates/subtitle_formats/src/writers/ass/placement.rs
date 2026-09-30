//! Where a cue sits on the canvas: the bottom band, or the top band when English lettered into the
//! picture fills the bottom one.
//!
//! **Role:** estimate each cue's box in the `Default` style and move the cue to the top
//! (`{\an8}`) when the box at the bottom meets on-screen writing shown during the cue.
//!
//! **Position:** called by `super::write_with`; uses `crate::cue`.
//!
//! **Signals and state:** none; the obstacles are given by the caller in canvas pixels.
//!
//! **Invariants:** with no obstacle active during a cue, the cue stays at the bottom; a cue moves
//! to the top only when the top box meets less writing than the bottom box (the bottom wins ties).

use crate::cue::{Cue, FrameRate};

use super::PLAY_RES;

/// The `Default` style's font size in canvas pixels.
const FONT_PX: f64 = 64.0;
/// The height one line takes: the font size with its line gap, outline and shadow.
const LINE_PX: f64 = 1.25 * FONT_PX;
/// The mean advance of one Arial character, as a share of the font size.
const CHAR_ADVANCE: f64 = 0.52;
/// The `Default` style's left and right margins.
const MARGIN_H: f64 = 120.0;
/// The `Default` style's vertical margin, used from the bottom edge and, moved up, from the top.
const MARGIN_V: f64 = 54.0;

/// On-screen writing a cue should not cover: shown from `start_s` to `end_s`, inside `rect`
/// (left, top, right, bottom in canvas pixels, 1920 by 1080).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Obstacle {
    pub start_s: f64,
    pub end_s: f64,
    pub rect: [f64; 4],
}

/// The band a cue is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Band {
    /// Bottom centre, the `Default` style's alignment 2.
    Bottom,
    /// Top centre, alignment 8.
    Top,
}

/// The band `cue` is drawn in: the top when the bottom box meets more of the writing active during
/// the cue than the top box does.
pub(super) fn band(cue: &Cue, rate: FrameRate, obstacles: &[Obstacle]) -> Band {
    let (start_s, end_s) = (rate.seconds(cue.start), rate.seconds(cue.end));
    let active: Vec<&Obstacle> = obstacles
        .iter()
        .filter(|o| o.start_s < end_s && o.end_s > start_s)
        .collect();
    if active.is_empty() {
        return Band::Bottom;
    }
    let covered =
        |rect: [f64; 4]| -> f64 { active.iter().map(|o| intersection_area(rect, o.rect)).sum() };
    let bottom = covered(cue_box(cue, Band::Bottom));
    if bottom <= 0.0 {
        return Band::Bottom;
    }
    if covered(cue_box(cue, Band::Top)) < bottom {
        Band::Top
    } else {
        Band::Bottom
    }
}

/// The estimated box of `cue` in `band`: centred, as wide as its longest line (at most the width
/// between the margins), one line height per line, stacked from the band's margin.
fn cue_box(cue: &Cue, band: Band) -> [f64; 4] {
    let (width, height) = (PLAY_RES.0 as f64, PLAY_RES.1 as f64);
    let longest = cue.lines.iter().map(|line| line.chars()).max().unwrap_or(0);
    let text_w = (longest as f64 * CHAR_ADVANCE * FONT_PX).min(width - 2.0 * MARGIN_H);
    let text_h = cue.lines.len() as f64 * LINE_PX;
    let left = (width - text_w) / 2.0;
    let (top, bottom) = match band {
        Band::Bottom => (height - MARGIN_V - text_h, height - MARGIN_V),
        Band::Top => (MARGIN_V, MARGIN_V + text_h),
    };
    [left, top, left + text_w, bottom]
}

/// The area two left-top-right-bottom rectangles share.
fn intersection_area(a: [f64; 4], b: [f64; 4]) -> f64 {
    let w = a[2].min(b[2]) - a[0].max(b[0]);
    let h = a[3].min(b[3]) - a[1].max(b[1]);
    if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
}

#[cfg(test)]
#[path = "tests/placement.rs"]
mod tests;
