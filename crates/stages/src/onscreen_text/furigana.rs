//! Furigana folded into the kanji line they annotate.
//!
//! **Role:** recognise a small kana-only line set just above a kanji line and keep it as that
//! line's ruby: its box is erased with the line and it is never translated or lettered alone.
//! **Position:** runs after reading consolidation in the read and translate stages, and after
//! [`super::unify`] joins the occurrences of one sign.
//! **Signals and state:** a mutable text document; no I/O.
//! **Invariants:** the base line keeps its reading, English, warnings, timing and geometry, and
//! the ruby's reading and English survive only in the base's reason; reviewed lines
//! are neither folded nor extended; a kana line that fits more than one base line stays separate;
//! folding twice changes nothing.

use super::read::{append_reason, observed_bounds};
use job_model::onscreen::{Quad, TextDocument, TextFrame, TextOccurrence};
use std::ops::RangeInclusive;

/// Ruby height as a share of the base line's height.
const HEIGHT_SHARE: RangeInclusive<f64> = 0.18..=0.55;
/// How far above the base line's top the ruby's bottom may sit, in base-line heights.
const MAX_LIFT: f64 = 0.5;
/// How far below the base line's top the ruby's bottom may reach, in base-line heights.
const MAX_OVERLAP: f64 = 0.35;
/// How far past the base line's ends the ruby's centre may sit, in base-line heights.
const SIDE_MARGIN: f64 = 0.1;
/// Ruby wider than the base line by this factor is a separate line, not a reading aid.
const MAX_WIDTH_SHARE: f64 = 1.6;
/// Ruby is a horizontal run: at least this wide relative to its height.
const MIN_ASPECT: f64 = 0.8;
/// Share of the ruby's span that must fall inside the base line's span.
const MIN_TIME_SHARE: f64 = 0.5;
/// Share of the shared time in which contemporary frames must place the kana as ruby.
const MIN_PLACED_SHARE: f64 = 2.0 / 3.0;
/// Two ruby boxes overlapping at least this much record the same reading.
const SAME_RUBY_OVERLAP: f64 = 0.8;
const EPSILON: f64 = 1e-6;

/// Ruby remains visual evidence of its base line, never a second independent translation.
pub(super) fn group_furigana(document: &mut TextDocument) {
    let mut groups = Vec::new();
    for (child_index, child) in document.occurrences.iter().enumerate() {
        if !kana_only(&child.japanese) || child.reviewed {
            continue;
        }
        let mut parents = document
            .occurrences
            .iter()
            .enumerate()
            .filter(|&(parent_index, parent)| {
                parent_index != child_index
                    && !parent.reviewed
                    && parent.japanese.chars().any(kanji)
                    && ruby_pair(child, parent)
            })
            .map(|(parent_index, _)| parent_index);
        if let (Some(parent), None) = (parents.next(), parents.next()) {
            groups.push((child_index, parent));
        }
    }
    let mut removed = vec![false; document.occurrences.len()];
    for (child, parent) in groups {
        let ruby = document.occurrences[child].clone();
        let base = &mut document.occurrences[parent];
        for crop in &ruby.crops {
            if !base.crops.contains(crop) {
                base.crops.push(crop.clone());
            }
        }
        let keyframe_s = base
            .keyframe
            .as_ref()
            .map(|keyframe| keyframe.time_s)
            .unwrap_or_else(|| (ruby.start_s.max(base.start_s) + ruby.end_s.min(base.end_s)) / 2.0);
        if let Some(frame) = nearest_frame(&ruby.frames, keyframe_s) {
            add_ruby(base, frame.quad);
        }
        for quad in &ruby.ruby {
            add_ruby(base, *quad);
        }
        let reason = format!(
            "Furigana evidence {}: {:?} ({:.2} confidence; {}; reference {:?}; English {:?}); base reading and geometry retained. {}",
            ruby.id,
            ruby.japanese,
            ruby.confidence,
            ruby.provenance.backend,
            ruby.provenance.reference,
            ruby.english,
            ruby.provenance.reason
        );
        append_reason(base, reason.trim());
        removed[child] = true;
    }
    let mut index = 0;
    document.occurrences.retain(|_| {
        let keep = !removed[index];
        index += 1;
        keep
    });
}

/// Records a ruby box on its base line unless an almost identical box is already there.
pub(super) fn add_ruby(base: &mut TextOccurrence, quad: Quad) {
    if quad.valid()
        && !base
            .ruby
            .iter()
            .any(|known| intersection_over_union(*known, quad) >= SAME_RUBY_OVERLAP)
    {
        base.ruby.push(quad);
    }
}

/// The frame showing `time_s`, or the one closest to it.
pub(super) fn nearest_frame(frames: &[TextFrame], time_s: f64) -> Option<&TextFrame> {
    let distance = |frame: &TextFrame| {
        if time_s < frame.time_s {
            frame.time_s - time_s
        } else if time_s >= frame.end_s {
            time_s - frame.end_s
        } else {
            0.0
        }
    };
    frames
        .iter()
        .min_by(|a, b| distance(a).total_cmp(&distance(b)))
}

/// Overlap of two quads' bounding boxes over their union; zero for invalid quads.
pub(super) fn intersection_over_union(a: Quad, b: Quad) -> f64 {
    if !a.valid() || !b.valid() {
        return 0.0;
    }
    let (al, at, ar, ab) = a.bounds();
    let (bl, bt, br, bb) = b.bounds();
    let intersection = (ar.min(br) - al.max(bl)).max(0.0) * (ab.min(bb) - at.max(bt)).max(0.0);
    let union = (ar - al) * (ab - at) + (br - bl) * (bb - bt) - intersection;
    intersection / union
}

pub(super) fn kana_only(text: &str) -> bool {
    let mut has_letter = false;
    text.chars().all(|c| {
        let kana =
            matches!(c,'\u{3041}'..='\u{3096}'|'\u{30a1}'..='\u{30fa}'|'\u{ff66}'..='\u{ff9d}');
        has_letter |= kana;
        kana || c.is_whitespace()
            || matches!(c, 'ー' | '\u{3099}' | '\u{309a}' | '\u{ff9e}' | '\u{ff9f}')
    }) && has_letter
}

fn kanji(c: char) -> bool {
    matches!(c,'\u{3400}'..='\u{9fff}'|'\u{f900}'..='\u{faff}')
}

/// The child shares most of its time with the parent, which shows no hole in that shared time,
/// and sits as ruby over the parent for most of it: a detector box that briefly covers only part
/// of the line does not break the pair.
fn ruby_pair(child: &TextOccurrence, parent: &TextOccurrence) -> bool {
    if !observed_bounds(child) || !observed_bounds(parent) {
        return false;
    }
    let (from, to) = (
        child.start_s.max(parent.start_s),
        child.end_s.min(parent.end_s),
    );
    if to - from < (child.end_s - child.start_s) * MIN_TIME_SHARE - EPSILON
        || parent.frames.windows(2).any(|pair| {
            pair[1].time_s > pair[0].end_s + EPSILON
                && pair[0].end_s < to - EPSILON
                && pair[1].time_s > from + EPSILON
        })
    {
        return false;
    }
    let (mut placed, mut compared) = (0.0, 0.0);
    for frame in &child.frames {
        for base in parent.frames.iter().filter(|base| {
            base.time_s < frame.end_s - EPSILON && base.end_s > frame.time_s + EPSILON
        }) {
            let shared = frame.end_s.min(base.end_s) - frame.time_s.max(base.time_s);
            compared += shared;
            if ruby_position(frame.quad, base.quad) {
                placed += shared;
            }
        }
    }
    compared > 0.0 && placed >= compared * MIN_PLACED_SHARE
}

fn ruby_position(child: Quad, parent: Quad) -> bool {
    if !child.valid() || !parent.valid() {
        return false;
    }
    let (cl, ct, cr, cb) = child.bounds();
    let (pl, pt, pr, pb) = parent.bounds();
    let (cw, ch, pw, ph) = (cr - cl, cb - ct, pr - pl, pb - pt);
    let level = |quad: Quad, height: f64| {
        (quad.0[1].y - quad.0[0].y).abs() < height * 0.25
            && (quad.0[2].y - quad.0[3].y).abs() < height * 0.25
    };
    let centre = (cl + cr) / 2.0;
    level(child, ch)
        && level(parent, ph)
        && HEIGHT_SHARE.contains(&(ch / ph))
        && cw >= ch * MIN_ASPECT
        && cw <= pw * MAX_WIDTH_SHARE
        && cb >= pt - ph * MAX_LIFT
        && cb <= pt + ph * MAX_OVERLAP
        && centre >= pl - ph * SIDE_MARGIN
        && centre <= pr + ph * SIDE_MARGIN
}

#[cfg(test)]
#[path = "tests/furigana.rs"]
mod tests;
