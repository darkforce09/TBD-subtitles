//! Region following in sample order: which writing continues, which starts and which ends, and
//! each occurrence's exact first and last frame once its group is bisected.
//!
//! **Role:** observe every sample's regions in frame order, start and extend occurrences, record
//! each entry and exit for bisection, apply the bisected frames, choose each occurrence's keyframe
//! and keep its candidates, then drop screening noise when the scan closes.
//! **Position:** owned by the coordinator, which feeds it the samples of one group at a time in
//! order and the bisected answers of that group before the next.
//! **Signals and state:** the document being built, the active regions with their anchors, the
//! frame indices of each occurrence until its keyframe is chosen, the last screened regions that
//! repeated samples reuse, and the keyframe candidates.
//! **Invariants:** every decision depends only on the samples in order, never on the order the
//! detector answers in; an occurrence's frames tile from its entry frame to its first absent
//! frame; its keyframe is the sample nearest the middle of its interval, the first on a tie; a
//! cut ends every active region.

use job_model::onscreen::{TextDocument, TextFrame};
use job_model::outputs::ShotChanges;
use media_io::yuv::Coefficients;
use rayon::prelude::*;

use super::flight::Regions;
use crate::onscreen_text::TextResult;
use crate::onscreen_text::detect::confirm::Closed;
use crate::onscreen_text::detect::crops::{has_text_contrast, picture_at};
use crate::onscreen_text::detect::probe::{Search, Seek, Transition};
use crate::onscreen_text::detect::regions::{
    Active, Observation, SAME_REGION, append_observation, associate, check_limits, crosses_cut,
    is_legible_size, overlap, plausible, same_signature, start_occurrence,
};
use crate::onscreen_text::detect::timing::{ScanStats, timed};
use crate::onscreen_text::detect::window::{Candidates, Group, HeldSample};

/// An occurrence's frame indices until its keyframe, `(frame position, frame index)`, is chosen.
#[derive(Default)]
struct Track {
    indices: Vec<u64>,
    /// Whether the first frame is the bisected entry frame rather than a sample.
    entered: bool,
    keyframe: Option<(usize, u64)>,
    /// Whether this occurrence has been observed on at least 2 samples or touched a cut.
    persistent: bool,
    /// The entry transition, deferred until the occurrence is confirmed persistent.
    pending_entry: Option<Transition>,
}

/// The region-following state of one scan.
pub(crate) struct Tracker<'a> {
    pub(crate) document: TextDocument,
    cuts: &'a ShotChanges,
    colour: Coefficients,
    active: Vec<Active>,
    tracks: Vec<Track>,
    previous_time: f64,
    observations: usize,
    last_regions: Regions,
    candidates: Candidates,
}

impl<'a> Tracker<'a> {
    pub(crate) fn new(
        document: TextDocument,
        cuts: &'a ShotChanges,
        colour: Coefficients,
        candidates: Candidates,
    ) -> Self {
        Self {
            document,
            cuts,
            colour,
            active: Vec::new(),
            tracks: Vec::new(),
            previous_time: f64::NEG_INFINITY,
            observations: 0,
            last_regions: Vec::new(),
            candidates,
        }
    }

    /// Follows one sample's regions, `screened` or, for a repeat, the last screened ones:
    /// matched regions continue, new ones start occurrences and unmatched active ones end; every
    /// start and end is recorded for bisection.
    pub(crate) fn observe(
        &mut self,
        sample: &HeldSample,
        screened: Option<Regions>,
        transitions: &mut Vec<Transition>,
        stats: &mut ScanStats,
    ) -> TextResult<()> {
        let frame = &sample.frame;
        let previous = sample.previous_sample();
        let regions = match screened {
            Some(regions) => {
                self.last_regions.clone_from(&regions);
                regions
            }
            None => self.last_regions.clone(),
        };
        if crosses_cut(self.cuts, self.previous_time, frame.time_s) {
            for ended in std::mem::take(&mut self.active) {
                let track = &mut self.tracks[ended.occurrence];
                if track.persistent {
                    if let Some(entry) = track.pending_entry.take() {
                        transitions.push(entry);
                    }
                    transitions.push(Transition::exit(previous, frame.index, ended));
                } else {
                    track.pending_entry = None;
                }
            }
        }
        self.previous_time = frame.time_s;
        let picture = frame
            .picture()
            .ok_or("A decoded frame does not hold its picture")?;
        let colour = self.colour;
        let current: Vec<Observation> = regions
            .iter()
            .copied()
            .filter(|&(quad, _)| {
                is_legible_size(quad) && has_text_contrast(&picture, &colour, quad)
            })
            .map(|(quad, confidence)| Observation {
                quad,
                confidence,
                surface_rgb: None,
            })
            .collect();
        self.observations += current.len();
        check_limits(self.observations, self.document.occurrences.len(), false)?;
        let needed: Vec<bool> = if current.is_empty() {
            vec![false; self.active.len()]
        } else {
            self.active
                .iter()
                .map(|prior| {
                    current
                        .iter()
                        .any(|obs| overlap(prior.quad, obs.quad) > SAME_REGION)
                })
                .collect()
        };
        let unchanged: Vec<bool> = if self.active.is_empty() || current.is_empty() {
            vec![false; self.active.len()]
        } else {
            timed(&mut stats.signature, || {
                self.active
                    .par_iter()
                    .zip(&needed)
                    .map(|(prior, &candidate)| {
                        candidate
                            && same_signature(
                                &prior.anchor,
                                &picture_at(&picture, &colour, prior.anchor_box),
                            )
                    })
                    .collect()
            })
        };
        let matches = associate(&self.active, &current, &unchanged);
        let mut unmatched: Vec<Option<Active>> = std::mem::take(&mut self.active)
            .into_iter()
            .map(Some)
            .collect();
        for (observation, matched) in current.into_iter().zip(matches) {
            let state = match matched.and_then(|index| unmatched[index].take()) {
                Some(mut state) => {
                    state.quad = observation.quad;
                    let track = &mut self.tracks[state.occurrence];
                    if !track.persistent {
                        track.persistent = true;
                        if let Some(entry) = track.pending_entry.take() {
                            transitions.push(entry);
                        }
                    }
                    state
                }
                None => {
                    check_limits(self.observations, self.document.occurrences.len(), true)?;
                    let occurrence =
                        start_occurrence(&mut self.document, frame.time_s, observation.confidence);
                    let anchor = timed(&mut stats.signature, || {
                        picture_at(&picture, &colour, observation.quad)
                    });
                    let entry = Transition {
                        occurrence,
                        search: Search::new(previous, frame.index, Seek::Entry),
                        quad: observation.quad,
                        anchor_box: observation.quad,
                        signature: anchor.clone(),
                    };
                    self.tracks.push(Track {
                        indices: Vec::new(),
                        entered: false,
                        keyframe: None,
                        persistent: false,
                        pending_entry: Some(entry),
                    });
                    // The anchor is immutable: a later picture cannot erase the reading evidence.
                    Active {
                        occurrence,
                        quad: observation.quad,
                        anchor,
                        anchor_box: observation.quad,
                    }
                }
            };
            append_observation(
                &mut self.document.occurrences[state.occurrence],
                &observation,
                frame.time_s,
                frame.end_s,
            );
            self.tracks[state.occurrence].indices.push(frame.index);
            self.candidates.offer(state.occurrence, frame);
            self.active.push(state);
        }
        for ended in unmatched.into_iter().flatten() {
            let track = &mut self.tracks[ended.occurrence];
            if track.persistent {
                transitions.push(Transition::exit(previous, frame.index, ended));
            } else {
                track.pending_entry = None;
            }
        }
        Ok(())
    }

    /// Starts each entering occurrence of `group` at its first present frame and ends each
    /// leaving one at its first absent frame, `answers` holding each transition's frame index;
    /// then releases the candidates the active occurrences can no longer choose.
    pub(crate) fn settle(
        &mut self,
        group: &Group,
        transitions: &[Transition],
        answers: &[u64],
    ) -> TextResult<()> {
        let mut exits = Vec::new();
        for (change, &index) in transitions.iter().zip(answers) {
            let frame = group
                .frame(index)
                .ok_or("A text transition lies outside its sample gap")?;
            match change.search.seek {
                Seek::Entry => self.enter(change.occurrence, frame.index, frame.time_s),
                Seek::Exit => exits.push((change.occurrence, frame.time_s)),
            }
        }
        for (occurrence, time_s) in exits {
            self.leave(occurrence, time_s);
        }
        if let Some(last) = group.samples.last() {
            let now_s = last.frame.time_s;
            for state in &self.active {
                let start_s = self.document.occurrences[state.occurrence].start_s;
                self.candidates
                    .prune(state.occurrence, (start_s + now_s) / 2.0);
            }
        }
        Ok(())
    }

    /// The most bytes the keyframe candidates held at once.
    pub(crate) fn keyframes_peak_bytes(&self) -> usize {
        self.candidates.peak_bytes()
    }

    /// Starts an occurrence at its entry frame, which carries the entering sample's geometry.
    fn enter(&mut self, occurrence: usize, index: u64, time_s: f64) {
        let item = &mut self.document.occurrences[occurrence];
        let track = &mut self.tracks[occurrence];
        let Some(first) = item.frames.first() else {
            return;
        };
        if track.indices.first().is_none_or(|&sample| index >= sample) {
            return;
        }
        let entry = TextFrame {
            time_s,
            end_s: first.time_s,
            quad: first.quad,
            confidence: first.confidence,
            surface_rgb: None,
        };
        item.frames.insert(0, entry);
        item.start_s = time_s;
        track.indices.insert(0, index);
        track.entered = true;
    }

    /// Ends an occurrence at its first absent frame and chooses its keyframe.
    fn leave(&mut self, occurrence: usize, time_s: f64) {
        let item = &mut self.document.occurrences[occurrence];
        if let Some(last) = item.frames.last_mut() {
            last.end_s = time_s;
        }
        item.end_s = time_s;
        self.choose_keyframe(occurrence);
    }

    /// The sample nearest the occurrence's midpoint becomes its keyframe; its candidate frame
    /// stays held when the window still holds it.
    fn choose_keyframe(&mut self, occurrence: usize) {
        let item = &self.document.occurrences[occurrence];
        let track = &mut self.tracks[occurrence];
        let middle = (item.start_s + item.end_s) / 2.0;
        let nearest = item
            .frames
            .iter()
            .enumerate()
            .skip(usize::from(track.entered))
            .min_by(|a, b| {
                (a.1.time_s - middle)
                    .abs()
                    .total_cmp(&(b.1.time_s - middle).abs())
            })
            .map(|(position, _)| position);
        track.keyframe =
            nearest.and_then(|position| Some((position, *track.indices.get(position)?)));
        track.indices = Vec::new();
        if let Some((_, index)) = track.keyframe {
            self.candidates.choose(occurrence, index);
        }
    }

    /// Ends the regions still shown on the final frame at its end, drops screening noise and
    /// hands out the document with each remaining occurrence's keyframe and the keyframes still
    /// held in memory.
    pub(crate) fn close(
        mut self,
        min_confirm_frames: usize,
        min_confirm_confidence: f64,
    ) -> Closed {
        for state in std::mem::take(&mut self.active) {
            let track = &mut self.tracks[state.occurrence];
            if track.persistent {
                let item = &mut self.document.occurrences[state.occurrence];
                if let Some(end_s) = item.frames.last().map(|frame| frame.end_s) {
                    item.end_s = end_s;
                }
                self.choose_keyframe(state.occurrence);
            }
        }
        let frame_area = f64::from(self.document.width) * f64::from(self.document.height);
        let mut keyframes = Vec::new();
        let occurrences = std::mem::take(&mut self.document.occurrences);
        let kept = occurrences
            .into_iter()
            .zip(&self.tracks)
            .filter(|(item, track)| track.persistent && plausible(item, frame_area))
            .map(|(item, track)| {
                keyframes.push(track.keyframe);
                item
            })
            .collect();
        self.document.occurrences = kept;
        let wanted: Vec<u64> = self
            .document
            .occurrences
            .iter()
            .zip(&keyframes)
            .filter(|(item, _)| {
                item.frames.len() >= min_confirm_frames && item.confidence >= min_confirm_confidence
            })
            .filter_map(|(_, kf)| kf.map(|(_, index)| index))
            .collect();
        Closed {
            document: self.document,
            keyframes,
            frames: self.candidates.into_frames(&wanted),
            min_confirm_frames,
            min_confirm_confidence,
        }
    }
}
