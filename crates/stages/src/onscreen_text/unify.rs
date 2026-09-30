//! Occurrences that show the same sign, joined into one.
//!
//! **Role:** merge the detector's and Claude's occurrences of one piece of writing, and pieces of
//! one sign split by a short miss, so the sign is erased and lettered once over one continuous
//! span.
//! **Position:** runs at the end of the translate stage and again after owner corrections in the
//! review step; both passes end with furigana grouping, so ruby that only Claude found folds into
//! the joined line.
//! **Signals and state:** a mutable text document and the job's shot cuts; no I/O.
//! **Invariants:** a reviewed occurrence is never absorbed and keeps its owner timing, wording
//! and presentation; a Claude box never replaces a detector quad; the joined frames tile the
//! joined span without a hole; writing never joins across a cut in a gap; a second pass changes
//! nothing.

use super::furigana::{self, add_ruby, intersection_over_union, nearest_frame};
use super::read::{append_reason, japanese};
use job_model::onscreen::{Quad, TextDocument, TextFrame, TextOccurrence};
use job_model::outputs::ShotChanges;
use std::cmp::Ordering;
use std::collections::HashMap;

/// The warning on writing that only Claude saw, dropped once a detector occurrence absorbs it.
pub(super) const FOUND_BY_CLAUDE: &str = "Found by Claude on the keyframe; the local detector did not see it; timing follows the surrounding event.";
/// The start of the warning on an uncertain translation, dropped with the English it describes.
pub(super) const TRANSLATION_NEEDS_REVIEW: &str = "Translation needs review:";
/// Keyframe boxes overlapping at least this much show the same place.
const MIN_BOX_OVERLAP: f64 = 0.3;
/// Growth of a box, in its heights, that still holds the other box's centre at the same place.
const CENTRE_MARGIN: f64 = 0.25;
/// Longest pause between two sightings of one sign inside a shot.
const MAX_GAP_S: f64 = 0.25;
/// The shorter reading contained in the longer needs at least this many characters.
const MIN_CONTAINED_CHARS: usize = 2;
const EPSILON: f64 = 1e-6;

/// Joins every group of occurrences that show the same sign; see the module invariants.
pub fn unify(document: &mut TextDocument, shots: &ShotChanges) {
    let items = &mut document.occurrences;
    let mut absorbed: HashMap<String, Vec<String>> = HashMap::new();
    let mut merged = true;
    while merged {
        merged = false;
        items.sort_by(|a, b| a.start_s.total_cmp(&b.start_s).then(a.id.cmp(&b.id)));
        let mut first = 0;
        while first < items.len() {
            let mut second = first + 1;
            while second < items.len()
                && items[second].start_s <= items[first].end_s + MAX_GAP_S + EPSILON
            {
                let Some(first_survives) = same_sign(&items[first], &items[second], shots) else {
                    second += 1;
                    continue;
                };
                let mut gone = items.remove(second);
                if !first_survives {
                    std::mem::swap(&mut items[first], &mut gone);
                }
                let mut ids = absorbed.remove(&gone.id).unwrap_or_default();
                ids.insert(0, gone.id.clone());
                absorbed
                    .entry(items[first].id.clone())
                    .or_default()
                    .extend(ids);
                absorb(&mut items[first], gone);
                merged = true;
                second = first + 1;
            }
            first += 1;
        }
    }
    for item in items.iter_mut() {
        if let Some(ids) = absorbed.get(&item.id) {
            append_reason(item, &format!("Same writing as {}.", ids.join(", ")));
        }
    }
    furigana::group_furigana(document);
}

/// Whether two occurrences show one sign, and if so whether the first one survives.
fn same_sign(a: &TextOccurrence, b: &TextOccurrence, shots: &ShotChanges) -> Option<bool> {
    if (a.reviewed && b.reviewed)
        || !same_writing(&a.japanese, &b.japanese)
        || !same_place(a, b)
        || !same_time(a, b, shots)
    {
        return None;
    }
    Some(survivor_order(a, b) != Ordering::Less)
}

/// A reviewed occurrence first, then detector geometry, then the longer span, then confidence.
fn survivor_order(a: &TextOccurrence, b: &TextOccurrence) -> Ordering {
    let span = |item: &TextOccurrence| item.end_s - item.start_s;
    a.reviewed
        .cmp(&b.reviewed)
        .then((!found_by_claude(&a.id)).cmp(&!found_by_claude(&b.id)))
        .then(span(a).total_cmp(&span(b)))
        .then(a.confidence.total_cmp(&b.confidence))
}

/// Ids of writing Claude found on a keyframe end in `-c` and a number.
fn found_by_claude(id: &str) -> bool {
    id.rsplit_once("-c")
        .is_some_and(|(_, number)| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
}

/// Equal readings, or one reading inside the other, once spaces are removed and small kana are
/// read as their full-size forms, which readers often confuse.
fn same_writing(a: &str, b: &str) -> bool {
    let normal = |text: &str| {
        text.chars()
            .filter(|c| !c.is_whitespace())
            .map(full_size_kana)
            .collect::<String>()
    };
    let (a, b) = (normal(a), normal(b));
    if !japanese(&a) || !japanese(&b) {
        return false;
    }
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    short == long || (short.chars().count() >= MIN_CONTAINED_CHARS && long.contains(&short))
}

fn full_size_kana(c: char) -> char {
    const SMALL: &str = "ぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶ";
    const FULL: &str = "あいうえおつやゆよわかけアイウエオツヤユヨワカケ";
    SMALL
        .chars()
        .position(|small| small == c)
        .and_then(|index| FULL.chars().nth(index))
        .unwrap_or(c)
}

fn same_place(a: &TextOccurrence, b: &TextOccurrence) -> bool {
    let (Some(a), Some(b)) = (keyframe_quad(a), keyframe_quad(b)) else {
        return false;
    };
    intersection_over_union(a, b) >= MIN_BOX_OVERLAP || centre_inside(a, b) || centre_inside(b, a)
}

/// Whether the centre of `inner` lies inside `outer` grown by a quarter of its height.
fn centre_inside(inner: Quad, outer: Quad) -> bool {
    let centre = inner.center();
    let (left, top, right, bottom) = outer.bounds();
    let margin = (bottom - top) * CENTRE_MARGIN;
    (left - margin..=right + margin).contains(&centre.x)
        && (top - margin..=bottom + margin).contains(&centre.y)
}

/// Spans that overlap, or a short pause with no shot cut in it.
fn same_time(a: &TextOccurrence, b: &TextOccurrence, shots: &ShotChanges) -> bool {
    let (early, late) = if a.start_s <= b.start_s {
        (a, b)
    } else {
        (b, a)
    };
    if early.end_s.min(late.end_s) - late.start_s > EPSILON {
        return true;
    }
    let gap = late.start_s - early.end_s;
    (-EPSILON..=MAX_GAP_S + EPSILON).contains(&gap)
        && !shots
            .cuts
            .iter()
            .any(|cut| cut.time_s >= early.end_s - EPSILON && cut.time_s <= late.start_s + EPSILON)
}

/// The quad observed at the keyframe, or at the middle of the span without one.
fn keyframe_quad(item: &TextOccurrence) -> Option<Quad> {
    let time_s = item
        .keyframe
        .as_ref()
        .map(|keyframe| keyframe.time_s)
        .unwrap_or((item.start_s + item.end_s) / 2.0);
    nearest_frame(&item.frames, time_s)
        .map(|frame| frame.quad)
        .filter(|quad| quad.valid())
}

/// Folds `gone` into `keep`: see the module invariants for what each side contributes. The
/// English with the higher confidence wins, with its reading and warnings; a reviewed `keep`
/// changes only its crops and ruby.
fn absorb(keep: &mut TextOccurrence, mut gone: TextOccurrence) {
    if !keep.reviewed {
        extend_frames(keep, &gone);
        if gone.english.is_some() && (keep.english.is_none() || gone.confidence > keep.confidence) {
            keep.japanese = std::mem::take(&mut gone.japanese);
            keep.english = gone.english.take();
            keep.confidence = gone.confidence;
            keep.provenance.reference = gone.provenance.reference.take();
            keep.warnings
                .retain(|warning| !warning.starts_with(TRANSLATION_NEEDS_REVIEW));
        } else {
            gone.warnings
                .retain(|warning| !warning.starts_with(TRANSLATION_NEEDS_REVIEW));
        }
        for warning in gone.warnings {
            if !keep.warnings.contains(&warning) {
                keep.warnings.push(warning);
            }
        }
        if !found_by_claude(&keep.id) {
            keep.warnings.retain(|warning| warning != FOUND_BY_CLAUDE);
        }
    }
    for crop in gone.crops {
        if !keep.crops.contains(&crop) {
            keep.crops.push(crop);
        }
    }
    for quad in gone.ruby {
        add_ruby(keep, quad);
    }
}

/// Adds the time only `gone` covers and tiles the frames over the joined span. A detector
/// occurrence brings its own quads; a Claude box brings only its time, shown with `keep`'s
/// keyframe quad.
fn extend_frames(keep: &mut TextOccurrence, gone: &TextOccurrence) {
    let fill = keyframe_quad(keep);
    let fill_rgb = keep
        .keyframe
        .as_ref()
        .and_then(|keyframe| nearest_frame(&keep.frames, keyframe.time_s))
        .and_then(|frame| frame.surface_rgb);
    let own_quads = !found_by_claude(&gone.id);
    let source = if gone.frames.is_empty() {
        fill.map(|quad| TextFrame {
            time_s: gone.start_s,
            end_s: gone.end_s,
            quad,
            confidence: gone.confidence,
            surface_rgb: fill_rgb,
        })
        .into_iter()
        .collect()
    } else {
        gone.frames.clone()
    };
    keep.frames.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
    let mut added = Vec::new();
    for frame in source {
        let (quad, surface_rgb) = match (own_quads, fill) {
            (true, _) => (frame.quad, frame.surface_rgb),
            (false, Some(quad)) => (quad, fill_rgb),
            (false, None) => continue,
        };
        for (time_s, end_s) in uncovered(&keep.frames, frame.time_s, frame.end_s) {
            added.push(TextFrame {
                time_s,
                end_s,
                quad,
                confidence: frame.confidence,
                surface_rgb,
            });
        }
    }
    keep.frames.extend(added);
    keep.frames.sort_by(|a, b| a.time_s.total_cmp(&b.time_s));
    keep.start_s = keep.start_s.min(gone.start_s);
    keep.end_s = keep.end_s.max(gone.end_s);
    for index in 1..keep.frames.len() {
        let next = keep.frames[index].time_s;
        let previous = &mut keep.frames[index - 1];
        if next > previous.end_s + EPSILON {
            previous.end_s = next;
        }
    }
    if let Some(first) = keep.frames.first_mut() {
        first.time_s = first.time_s.min(keep.start_s);
    }
    if let Some(last) = keep.frames.last_mut() {
        last.end_s = last.end_s.max(keep.end_s);
    }
}

/// The parts of `from..to` that no frame covers, in order.
fn uncovered(frames: &[TextFrame], from: f64, to: f64) -> Vec<(f64, f64)> {
    let mut parts = Vec::new();
    let mut cursor = from;
    for frame in frames {
        if frame.end_s <= cursor + EPSILON {
            continue;
        }
        if frame.time_s >= to - EPSILON {
            break;
        }
        if frame.time_s > cursor + EPSILON {
            parts.push((cursor, frame.time_s));
        }
        cursor = cursor.max(frame.end_s);
    }
    if cursor < to - EPSILON {
        parts.push((cursor, to));
    }
    parts
}

#[cfg(test)]
#[path = "tests/unify.rs"]
mod tests;
