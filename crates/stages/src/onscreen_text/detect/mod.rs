//! Coarse-to-fine detection of visible writing on a screening copy of the video.
//!
//! **Role:** screen half-second samples of a small proxy stream, follow text regions between
//! samples, narrow each region's first and last frame exactly by bisecting the frames between
//! samples, then confirm every occurrence on one full-resolution still.
//! **Position:** first visual stage; reads a `FrameSource` and a `TextDetection`, and writes crops
//! and keyframe stills under the job's `visual/` folder.
//! **Signals and state:** a pending batch of at most eight samples with the frames between them,
//! the active regions with fixed anchors, the last screened picture, a probe cache cleared per
//! batch and at most four stills.
//! **Invariants:** document quads are in source pixels; an occurrence's frames tile from its entry
//! frame to its first absent frame; a gap never crosses a shot cut; the limits fail explicitly
//! instead of dropping text; no full-video image extraction occurs.

mod crops;
mod regions;
mod screen;
mod source;

use std::collections::HashMap;
use std::path::Path;

use image::{GrayImage, RgbImage};
use inference::ocr::TextDetection;
use job_model::onscreen::{Point, Quad, TextDocument, TextFrame};
use job_model::outputs::{ShotChanges, VideoStream};

use super::TextResult;
use crops::confirm_keyframes;
use regions::{
    Active, Observation, SAME_REGION, append_observation, associate, check_limits, crosses_cut,
    overlap, same_signature, signature, start_occurrence,
};
use screen::{
    Pending, PendingSample, SCREEN_BATCH, Samples, Search, Seek, bisect, near_duplicate,
    sample_step,
};

pub use crops::crop;
pub use source::{FfmpegSource, FrameSource, ProxyFrame};

/// Rows of the screening copy; the width keeps the source aspect.
pub const PROXY_LINES: u32 = 360;

/// Probe regions of one frame: source-pixel quads with their proxy signatures.
type Probes = HashMap<u64, Vec<(Quad, GrayImage)>>;

/// Finds every text occurrence of the video with exact frame boundaries, one crop and one
/// keyframe still each.
pub fn scan(
    source: &mut dyn FrameSource,
    stream: &VideoStream,
    cuts: &ShotChanges,
    root: &Path,
    detector: &mut dyn TextDetection,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<TextDocument> {
    let (proxy_width, proxy_height) = source.proxy_size();
    if stream.width == 0 || stream.height == 0 || proxy_width == 0 || proxy_height == 0 {
        return Err(
            "The visual scan needs a video and a screening copy with a picture size".into(),
        );
    }
    let step = sample_step(frame_rate(stream));
    let samples = Samples::new(source.timeline(), cuts, step);
    let frame_count = source.timeline().len() as u64;
    let mut scanner = Scanner {
        document: TextDocument {
            width: stream.width,
            height: stream.height,
            proxy_width,
            sample_step: u32::try_from(step).unwrap_or(u32::MAX),
            ..TextDocument::default()
        },
        cuts,
        scale: f64::from(stream.width) / f64::from(proxy_width),
        active: Vec::new(),
        tracks: Vec::new(),
        last_screen: None,
        previous_time: f64::NEG_INFINITY,
        observations: 0,
    };
    let mut pending = Pending::default();
    while let Some(frame) = source.next_proxy()? {
        if frame.index != scanner.document.decoded_frames
            || frame.index >= frame_count
            || frame.rgb.dimensions() != (proxy_width, proxy_height)
        {
            return Err("The screening copy does not match the video's frame timeline".into());
        }
        scanner.document.decoded_frames += 1;
        let sample = samples.contains(frame.index);
        if let Some(batch) = pending.push(frame, sample) {
            scanner.process(&batch, detector)?;
        }
        debug_assert!(pending.frames() <= SCREEN_BATCH * (step as usize + 1));
        progress(scanner.document.decoded_frames as usize, 0);
    }
    source.finish_proxies()?;
    if scanner.document.decoded_frames != frame_count {
        return Err("The screening copy ended before the video's final frame".into());
    }
    scanner.process(&pending.finish()?, detector)?;
    let (mut document, keyframes) = scanner.close();
    confirm_keyframes(&mut document, &keyframes, source, detector, root, progress)?;
    Ok(document)
}

/// The stream's frame rate, or 24 when it reports none.
fn frame_rate(stream: &VideoStream) -> f64 {
    stream
        .fps()
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .unwrap_or(24.0)
}

/// A proxy quad in source pixels.
fn scaled(quad: Quad, scale: f64) -> Quad {
    Quad(quad.0.map(|point| Point {
        x: point.x * scale,
        y: point.y * scale,
    }))
}

/// Where an occurrence enters or leaves between two samples, and what identifies it there.
struct Transition {
    occurrence: usize,
    /// The batch position of the sample whose gap holds the change.
    sample: usize,
    search: Search,
    quad: Quad,
    signature: GrayImage,
}

impl Transition {
    fn exit(sample: usize, previous: u64, index: u64, ended: Active) -> Self {
        Self {
            occurrence: ended.occurrence,
            sample,
            search: Search::new(previous, index, Seek::Exit),
            quad: ended.quad,
            signature: ended.anchor,
        }
    }
}

/// An occurrence's frame indices until its keyframe, `(frame position, frame index)`, is chosen.
#[derive(Default)]
struct Track {
    indices: Vec<u64>,
    keyframe: Option<(usize, u64)>,
}

/// The last picture the detector screened and its regions, in proxy pixels.
struct Screened {
    image: RgbImage,
    regions: Vec<(Quad, f64)>,
}

struct Scanner<'a> {
    document: TextDocument,
    cuts: &'a ShotChanges,
    scale: f64,
    active: Vec<Active>,
    tracks: Vec<Track>,
    last_screen: Option<Screened>,
    previous_time: f64,
    observations: usize,
}

impl Scanner<'_> {
    /// Screens a batch of samples, follows their regions in frame order and narrows every entry
    /// and exit to its frame.
    fn process(
        &mut self,
        batch: &[PendingSample],
        detector: &mut dyn TextDetection,
    ) -> TextResult<()> {
        let screens = self.screen(batch, detector)?;
        let mut transitions = Vec::new();
        for (position, (sample, regions)) in batch.iter().zip(&screens).enumerate() {
            self.observe(position, sample, regions, &mut transitions)?;
        }
        self.resolve(batch, transitions, detector)
    }

    /// Each sample's regions in proxy pixels, from one detector call. A sample that nearly
    /// repeats the last screened picture reuses its regions; comparing with that picture rather
    /// than the previous sample keeps a slow change from drifting through a chain of repeats.
    fn screen(
        &mut self,
        batch: &[PendingSample],
        detector: &mut dyn TextDetection,
    ) -> TextResult<Vec<Vec<(Quad, f64)>>> {
        let mut images = Vec::new();
        let mut sources = Vec::with_capacity(batch.len());
        let mut last = self.last_screen.as_ref().map(|screened| &screened.image);
        for sample in batch {
            if !last.is_some_and(|image| near_duplicate(image, &sample.frame.rgb)) {
                images.push(sample.frame.rgb.clone());
                last = Some(&sample.frame.rgb);
            }
            sources.push(images.len().checked_sub(1));
        }
        let screened = if images.is_empty() {
            Vec::new()
        } else {
            detector.screen_batch(&images)?
        };
        if screened.len() != images.len() {
            return Err("The text detector returned a different number of screens".into());
        }
        let carried = self
            .last_screen
            .as_ref()
            .map(|screened| screened.regions.clone())
            .unwrap_or_default();
        let regions = sources
            .iter()
            .map(|source| source.map_or_else(|| carried.clone(), |slot| screened[slot].clone()))
            .collect();
        if let (Some(image), Some(found)) = (images.pop(), screened.last()) {
            self.last_screen = Some(Screened {
                image,
                regions: found.clone(),
            });
        }
        Ok(regions)
    }

    /// Follows one sample's regions: matched regions continue, new ones start occurrences and
    /// unmatched active ones end; every start and end is recorded for bisection.
    fn observe(
        &mut self,
        position: usize,
        sample: &PendingSample,
        regions: &[(Quad, f64)],
        transitions: &mut Vec<Transition>,
    ) -> TextResult<()> {
        let frame = &sample.frame;
        let previous = sample.previous_sample();
        if crosses_cut(self.cuts, self.previous_time, frame.time_s) {
            for ended in std::mem::take(&mut self.active) {
                transitions.push(Transition::exit(position, previous, frame.index, ended));
            }
        }
        self.previous_time = frame.time_s;
        let current: Vec<Observation> = regions
            .iter()
            .map(|&(quad, confidence)| Observation {
                quad: scaled(quad, self.scale),
                confidence,
                signature: signature(&crop(&frame.rgb, quad)),
                surface_rgb: None,
            })
            .collect();
        self.observations += current.len();
        check_limits(self.observations, self.document.occurrences.len(), false)?;
        let matches = associate(&self.active, &current);
        let mut unmatched: Vec<Option<Active>> = std::mem::take(&mut self.active)
            .into_iter()
            .map(Some)
            .collect();
        for (observation, matched) in current.into_iter().zip(matches) {
            let state = match matched.and_then(|index| unmatched[index].take()) {
                Some(mut state) => {
                    state.quad = observation.quad;
                    state
                }
                None => {
                    check_limits(self.observations, self.document.occurrences.len(), true)?;
                    let occurrence =
                        start_occurrence(&mut self.document, frame.time_s, observation.confidence);
                    self.tracks.push(Track::default());
                    transitions.push(Transition {
                        occurrence,
                        sample: position,
                        search: Search::new(previous, frame.index, Seek::Entry),
                        quad: observation.quad,
                        signature: observation.signature.clone(),
                    });
                    // The anchor is immutable: a later picture cannot erase the reading evidence.
                    Active {
                        occurrence,
                        quad: observation.quad,
                        anchor: observation.signature.clone(),
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
            self.active.push(state);
        }
        for ended in unmatched.into_iter().flatten() {
            transitions.push(Transition::exit(position, previous, frame.index, ended));
        }
        Ok(())
    }

    /// Narrows the batch's transitions in lockstep, one screening call per step, then starts
    /// each entering occurrence at its first present frame and ends each leaving one at its
    /// first absent frame.
    fn resolve(
        &mut self,
        batch: &[PendingSample],
        transitions: Vec<Transition>,
        detector: &mut dyn TextDetection,
    ) -> TextResult<()> {
        let mut searches: Vec<Search> = transitions.iter().map(|change| change.search).collect();
        let scale = self.scale;
        let mut probes = Probes::new();
        bisect(
            &mut searches,
            &mut probes,
            |probes, indices| probe(batch, indices, scale, &mut *detector, probes),
            |probes, change, index| {
                present(&transitions[change], probes.get(&index).map(Vec::as_slice))
            },
        )?;
        let mut exits = Vec::new();
        for (change, search) in transitions.iter().zip(&searches) {
            let frame = batch[change.sample]
                .frame(search.hi)
                .ok_or("A text transition lies outside its sample gap")?;
            match search.seek {
                Seek::Entry => self.enter(change.occurrence, frame.index, frame.time_s),
                Seek::Exit => exits.push((change.occurrence, frame.time_s)),
            }
        }
        for (occurrence, time_s) in exits {
            self.leave(occurrence, time_s);
        }
        Ok(())
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

    /// The observed frame nearest the occurrence's midpoint becomes its keyframe.
    fn choose_keyframe(&mut self, occurrence: usize) {
        let item = &self.document.occurrences[occurrence];
        let middle = (item.start_s + item.end_s) / 2.0;
        let nearest = item
            .frames
            .iter()
            .enumerate()
            .min_by(|a, b| {
                (a.1.time_s - middle)
                    .abs()
                    .total_cmp(&(b.1.time_s - middle).abs())
            })
            .map(|(position, _)| position);
        let track = &mut self.tracks[occurrence];
        track.keyframe =
            nearest.and_then(|position| Some((position, *track.indices.get(position)?)));
        track.indices = Vec::new();
    }

    /// Ends the regions still shown on the final frame at its end and hands out the document with
    /// each occurrence's keyframe.
    fn close(mut self) -> (TextDocument, Vec<Option<(usize, u64)>>) {
        for state in std::mem::take(&mut self.active) {
            let item = &mut self.document.occurrences[state.occurrence];
            if let Some(end_s) = item.frames.last().map(|frame| frame.end_s) {
                item.end_s = end_s;
            }
            self.choose_keyframe(state.occurrence);
        }
        let keyframes = self.tracks.iter().map(|track| track.keyframe).collect();
        (self.document, keyframes)
    }
}

/// Screens the probe frames not yet cached, in one detector call.
fn probe(
    batch: &[PendingSample],
    indices: &[u64],
    scale: f64,
    detector: &mut dyn TextDetection,
    cache: &mut Probes,
) -> TextResult<()> {
    let mut frames = Vec::new();
    for index in indices {
        if !cache.contains_key(index) {
            frames.push(
                batch
                    .iter()
                    .find_map(|sample| sample.frame(*index))
                    .ok_or("A bisection probe lies outside the pending frames")?,
            );
        }
    }
    if frames.is_empty() {
        return Ok(());
    }
    // Every transition of the batch probes at once, so the call is split to the screening batch
    // size to keep the detector's memory bounded.
    let mut screens = Vec::with_capacity(frames.len());
    for chunk in frames.chunks(SCREEN_BATCH) {
        let images: Vec<RgbImage> = chunk.iter().map(|frame| frame.rgb.clone()).collect();
        let screened = detector.screen_batch(&images)?;
        if screened.len() != images.len() {
            return Err("The text detector returned a different number of screens".into());
        }
        screens.extend(screened);
    }
    for (frame, regions) in frames.into_iter().zip(screens) {
        let found = regions
            .into_iter()
            .map(|(quad, _)| (scaled(quad, scale), signature(&crop(&frame.rgb, quad))))
            .collect();
        cache.insert(frame.index, found);
    }
    Ok(())
}

/// Whether a probed frame shows the transition's region: an overlapping region whose signature
/// matches the region's.
fn present(change: &Transition, probe: Option<&[(Quad, GrayImage)]>) -> bool {
    probe.is_some_and(|regions| {
        regions.iter().any(|(quad, picture)| {
            overlap(*quad, change.quad) > SAME_REGION && same_signature(&change.signature, picture)
        })
    })
}

#[cfg(test)]
#[path = "tests/scan.rs"]
mod tests;
