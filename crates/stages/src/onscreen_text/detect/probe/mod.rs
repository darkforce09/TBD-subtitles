//! Bisection probes: the exact frame where a region enters or leaves between two samples.
//!
//! **Role:** name each transition the coordinator finds, convert the held frames a bisection
//! step probes to padded pictures, have them screened ahead of the waiting screening batches,
//! and decide on each probed frame whether the transition's region is still shown.
//! **Position:** called by the coordinator once per group, after the group is observed; the
//! caller supplies how probe pictures are screened.
//! **Signals and state:** a cache of probed frames' regions for one group.
//! **Invariants:** probes come from the frames the group holds, never a new decode; a frame is
//! screened at most once per group; a region is present on a frame when a screened box overlaps
//! it by more than 0.45 and the frame's luma at the region's anchor box still matches the anchor.

mod search;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use image::GrayImage;
use inference::ocr::pool::PaddedFrame;
use job_model::onscreen::Quad;
use media_io::yuv::{Coefficients, YuvFrame};
use rayon::prelude::*;

use super::crops::picture_at;
use super::regions::{Active, SAME_REGION, overlap, same_signature};
use super::screen::padded;
use super::timing::ScanStats;
use super::window::Group;
use crate::onscreen_text::TextResult;

pub(crate) use search::{Search, Seek, bisect};

/// Screens probe pictures at the probes' priority and answers each one's regions in order.
pub(crate) type ProbeScreen<'a> =
    dyn FnMut(Vec<PaddedFrame>) -> TextResult<Vec<Vec<(Quad, f64)>>> + 'a;

/// Where an occurrence enters or leaves between two samples, and what identifies it there: its
/// box for overlap, its anchor box and the anchor's signature.
pub(crate) struct Transition {
    pub(crate) occurrence: usize,
    pub(crate) search: Search,
    pub(crate) quad: Quad,
    pub(crate) anchor_box: Quad,
    pub(crate) signature: GrayImage,
}

impl Transition {
    /// The region `ended` leaves somewhere after sample `previous` and by sample `index`.
    pub(crate) fn exit(previous: u64, index: u64, ended: Active) -> Self {
        Self {
            occurrence: ended.occurrence,
            search: Search::new(previous, index, Seek::Exit),
            quad: ended.quad,
            anchor_box: ended.anchor_box,
            signature: ended.anchor,
        }
    }
}

/// The frames probed in one group: their screened boxes, the time spent getting them and how
/// many there were.
#[derive(Default)]
struct Probed {
    regions: HashMap<u64, Vec<Quad>>,
    spent: Duration,
    frames: u64,
}

/// Narrows every transition of `group` to its frame: the first present frame of an entry, the
/// first absent frame of a leave, one per transition in order.
pub(crate) fn narrow(
    transitions: &[Transition],
    group: &Group,
    colour: &Coefficients,
    screen: &mut ProbeScreen<'_>,
    stats: &mut ScanStats,
) -> TextResult<Vec<u64>> {
    let started = Instant::now();
    let mut searches: Vec<Search> = transitions.iter().map(|change| change.search).collect();
    let mut cache = Probed::default();
    bisect(
        &mut searches,
        &mut cache,
        |probed, indices| {
            let probing = Instant::now();
            let wanted: Vec<u64> = indices
                .iter()
                .copied()
                .filter(|index| !probed.regions.contains_key(index))
                .collect();
            let frames = wanted
                .iter()
                .map(|&index| {
                    group
                        .frame(index)
                        .ok_or_else(|| "A bisection probe lies outside the held frames".into())
                })
                .collect::<TextResult<Vec<&YuvFrame>>>()?;
            let pictures = frames
                .par_iter()
                .map(|frame| {
                    frame
                        .picture()
                        .map(|picture| padded(&picture, colour))
                        .ok_or_else(|| "A held frame does not hold its picture".to_string())
                })
                .collect::<Result<Vec<PaddedFrame>, String>>()?;
            if !pictures.is_empty() {
                let screens = screen(pictures)?;
                if screens.len() != wanted.len() {
                    return Err("The text detector returned a different number of screens".into());
                }
                for (index, regions) in wanted.iter().zip(screens) {
                    let quads = regions.into_iter().map(|(quad, _)| quad).collect();
                    probed.regions.insert(*index, quads);
                }
            }
            probed.frames += wanted.len() as u64;
            probed.spent += probing.elapsed();
            Ok(())
        },
        |probed, change, index| {
            let change = &transitions[change];
            group.frame(index).is_some_and(|frame| {
                present(
                    change,
                    frame,
                    probed.regions.get(&index).map(Vec::as_slice),
                    colour,
                )
            })
        },
    )?;
    stats.probe += cache.spent;
    stats.signature += started.elapsed().saturating_sub(cache.spent);
    stats.frames_probed += cache.frames;
    stats.frames_screened += cache.frames;
    Ok(searches.iter().map(|search| search.hi).collect())
}

/// Whether a probed frame shows the transition's region: a screened box overlaps it and the
/// frame's luma at the region's anchor box still matches the anchor.
fn present(
    change: &Transition,
    frame: &YuvFrame,
    probe: Option<&[Quad]>,
    colour: &Coefficients,
) -> bool {
    probe.is_some_and(|quads| {
        quads
            .iter()
            .any(|quad| overlap(*quad, change.quad) > SAME_REGION)
    }) && frame.picture().is_some_and(|picture| {
        same_signature(
            &change.signature,
            &picture_at(&picture, colour, change.anchor_box),
        )
    })
}

#[cfg(test)]
#[path = "tests/probe.rs"]
mod tests;
