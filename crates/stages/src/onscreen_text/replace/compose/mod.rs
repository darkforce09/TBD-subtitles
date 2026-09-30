//! English lettering composed onto inpainted plates.
//!
//! **Role:** letter every pending occurrence's English in the original writing's place, colour
//! and weight, and write one RGBA patch per plate for the localized video.
//! **Position:** the compose step of in-place replacement, after inpainting and before the
//! localized video is encoded; runs on the CPU.
//! **Signals and state:** the inpainted `ReplacementDocument`, the reviewed `TextDocument`, the
//! `latin-fonts` model folder; patches and previews under `visual/patches/<id>/`.
//! **Invariants:** only `Pending` occurrences change; each ends `Baked` with a patch on every
//! plate or `Fallback` with a reason and no patches; members of one container keep their
//! original size ratio; output is deterministic.

mod bake;
mod colours;
mod containers;
mod font;
mod layout;
mod patch;
mod render;
mod warp;

use std::collections::HashMap;
use std::path::Path;

use job_model::onscreen::{
    LetteringStyle, Quad, ReplaceStatus, ReplacedText, ReplacementDocument, TextDocument,
    TextOccurrence,
};

use self::containers::Member;
use self::font::{FontMetrics, LetteringFont};
use self::layout::{Area, Layout};
use crate::onscreen_text::TextResult;

/// The smallest cap height, in pixels of a 1080-line frame, that stays readable.
const MIN_CAP_PX_1080: f64 = 14.0;
/// The reason recorded when the lettering would be too small.
pub(crate) const TOO_SMALL: &str = "The English would be too small to read in place";

/// Letter every inpainted occurrence in English; `fonts` is the `latin-fonts` model folder.
pub fn compose(
    document: &mut ReplacementDocument,
    text: &TextDocument,
    root: &Path,
    fonts: &Path,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<()> {
    let font = LetteringFont::open(fonts)?;
    let frame_height = if document.height > 0 {
        document.height
    } else {
        text.height
    };
    let min_cap = MIN_CAP_PX_1080 * f64::from(frame_height) / 1080.0;
    let occurrences: HashMap<&str, &TextOccurrence> = text
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.id.as_str(), occurrence))
        .collect();
    let pending: Vec<usize> = (0..document.texts.len())
        .filter(|&index| document.texts[index].status == ReplaceStatus::Pending)
        .collect();
    let total = pending.len();
    let mut done = 0;
    progress(done, total);
    let mut prepared = Vec::new();
    for index in pending {
        let item = &mut document.texts[index];
        item.container = None;
        match prepare(item, occurrences.get(item.id.as_str()).copied(), &font) {
            Ok(ready) => prepared.push(Prepared { index, ..ready }),
            Err(reason) => {
                bake::fall_back(root, item, reason)?;
                done += 1;
                progress(done, total);
            }
        }
    }
    let members: Vec<Member> = prepared.iter().map(|ready| ready.member).collect();
    for group in containers::group(&members) {
        if group.len() > 1 {
            let container = document.texts[prepared[group[0]].index].id.clone();
            for &position in &group {
                document.texts[prepared[position].index].container = Some(container.clone());
            }
        }
        let fitted = fit_group(&font, &prepared, &group, min_cap)?;
        for (&position, (metrics, layout)) in group.iter().zip(fitted) {
            let ready = &prepared[position];
            let item = &mut document.texts[ready.index];
            let outcome = match layout {
                Some(layout) => bake::bake(root, item, ready, &metrics, &layout)?,
                None => Err(TOO_SMALL.to_string()),
            };
            if let Err(reason) = outcome {
                bake::fall_back(root, item, reason)?;
            }
            done += 1;
            progress(done, total);
        }
    }
    Ok(())
}

/// A pending occurrence with everything its lettering needs.
pub(crate) struct Prepared {
    pub index: usize,
    pub english: String,
    pub style: LetteringStyle,
    /// The writing's quad at the keyframe, in source pixels.
    pub quad: Quad,
    pub area: Area,
    /// The frame index of the keyframe.
    pub keyframe: u64,
    pub member: Member,
}

/// Collect an occurrence's inputs, or the reason it cannot be lettered.
fn prepare(
    item: &ReplacedText,
    occurrence: Option<&TextOccurrence>,
    font: &LetteringFont,
) -> Result<Prepared, String> {
    let occurrence = occurrence.ok_or("The writing is no longer in the reviewed text document")?;
    let style = item
        .style
        .clone()
        .ok_or("The original lettering style was not measured")?;
    let english = occurrence
        .english
        .as_deref()
        .map(str::trim)
        .filter(|english| !english.is_empty())
        .ok_or("The writing has no English translation")?
        .to_string();
    if let Some(c) = font.missing_glyph(&english) {
        return Err(format!("The font has no glyph for “{c}”"));
    }
    if item.plates.is_empty() {
        return Err("The writing has no background plates".to_string());
    }
    let quad = containers::keyframe_frame(occurrence)
        .map(|frame| frame.quad)
        .ok_or("The writing has no tracked position at its keyframe")?;
    Ok(Prepared {
        index: 0,
        area: containers::rectified(quad),
        keyframe: containers::keyframe_index(item, occurrence),
        member: Member {
            first_frame: item.first_frame,
            last_frame: item.last_frame,
            quad,
            line_height: style.line_height_px,
        },
        english,
        style,
        quad,
    })
}

/// Layouts for one container: its members share one size ratio; a member that cannot reach
/// the readable size gets `None`.
fn fit_group<'f>(
    font: &'f LetteringFont,
    prepared: &[Prepared],
    group: &[usize],
    min_cap: f64,
) -> TextResult<Vec<(FontMetrics<'f>, Option<Layout>)>> {
    let mut fitted = Vec::with_capacity(group.len());
    for &position in group {
        let ready = &prepared[position];
        let metrics = font.metrics(layout::weight(&ready.style))?;
        let target = layout::target_cap(&ready.style);
        let best = layout::fit(&metrics, &ready.english, ready.area, target, min_cap);
        fitted.push((metrics, target, best));
    }
    let ratios: Vec<(f64, f64)> = fitted
        .iter()
        .filter_map(|(_, target, best)| best.as_ref().map(|layout| (*target, layout.cap)))
        .collect();
    let scale = layout::shared_scale(&ratios);
    let single = group.len() == 1;
    Ok(fitted
        .into_iter()
        .zip(group)
        .map(|((metrics, target, best), &position)| {
            let layout = best.and_then(|best| {
                if single {
                    return Some(best);
                }
                let cap = target * scale;
                if cap < min_cap * (1.0 - 1e-9) {
                    return None;
                }
                let ready = &prepared[position];
                Some(layout::fit_at(&metrics, &ready.english, ready.area, cap).unwrap_or(best))
            });
            (metrics, layout)
        })
        .collect())
}

#[cfg(test)]
#[path = "tests/compose.rs"]
mod tests;
