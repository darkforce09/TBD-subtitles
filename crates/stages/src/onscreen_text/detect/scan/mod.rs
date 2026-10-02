//! The in-order coordinator: decoding, screening ahead, and observation strictly in sample order.
//!
//! **Role:** read every decoded frame, choose and convert the samples to screen, submit them in
//! batches as soon as a batch fills while decoding goes on, and apply the results group by group
//! in sample order, bisecting each group's transitions before the next group is observed.
//! **Position:** called by `detect::scan_measured`; owns the flight of screening jobs, the group
//! being gathered, the closed groups waiting for their results, and the region tracker.
//! **Signals and state:** at most `GROUPS_PER_SESSION` closed groups per screening session wait
//! with their frames; the last screened sample's thumbnail; the stats.
//! **Invariants:** every frame of the timeline is decoded once, in order, at the video's size;
//! groups are observed strictly in order and only once their job is answered, so one and two
//! sessions, or results arriving in any order, give the same document; held frames are bounded
//! by the waiting groups and the keyframe candidates' budget; progress is reported once per
//! `PROGRESS_FRAMES` frames and at the last frame.

mod flight;
mod tracker;

use std::collections::VecDeque;
use std::path::Path;

use inference::ocr::pool::{PaddedFrame, Priority, TextScreening};
use job_model::onscreen::TextDocument;
use job_model::outputs::{ShotChanges, VideoStream};
use media_io::yuv::{Coefficients, YuvFrame};

use super::confirm::confirm_keyframes;
use super::probe::{Transition, narrow};
use super::screen::{Repeats, Samples, padded, sample_step};
use super::source::FrameSource;
use super::timing::{ScanStats, timed};
use super::window::{Candidates, Gathering, Group, HeldSample};
use super::{ScanLimits, frame_rate};
use crate::onscreen_text::TextResult;
use flight::Flight;
use tracker::Tracker;

pub(crate) use flight::Regions;

/// Closed groups that may wait for their results, per screening session.
const GROUPS_PER_SESSION: usize = 6;
/// Frames between two progress reports while decoding.
const PROGRESS_FRAMES: u64 = 24;

/// The whole scan: screening, bisection and confirmation, with where its time went.
pub(crate) fn run(
    source: &mut dyn FrameSource,
    stream: &VideoStream,
    cuts: &ShotChanges,
    root: &Path,
    pool: &mut dyn TextScreening,
    progress: &(dyn Fn(usize, usize) + Sync),
    limits: ScanLimits,
) -> TextResult<(TextDocument, ScanStats)> {
    let size = source.frame_size();
    if stream.width == 0 || stream.height == 0 || size != (stream.width, stream.height) {
        return Err("The visual scan needs decoded frames at the video's picture size".into());
    }
    let colour = Coefficients::of(stream);
    let step = sample_step(frame_rate(stream));
    let document = TextDocument {
        width: stream.width,
        height: stream.height,
        proxy_width: stream.width,
        sample_step: u32::try_from(step).unwrap_or(u32::MAX),
        ..TextDocument::default()
    };
    let mut stats = ScanStats::default();
    let tracker = Tracker::new(
        document,
        cuts,
        colour,
        Candidates::new(limits.candidate_budget),
        limits.min_bisection_samples,
    );
    let batch = pool.shape().batch.max(1);
    let most_waiting = GROUPS_PER_SESSION * pool.sessions().max(1);
    let mut screening = Screening {
        flight: Flight::new(pool),
        tracker,
        colour,
        batch,
        waiting: VecDeque::new(),
        prior_sample: None,
    };
    let samples = Samples::new(source.timeline(), cuts, step);
    let frame_count = source.timeline().len() as u64;
    let mut gathering = Gathering::default();
    let mut repeats = Repeats::default();
    let mut decoded = 0u64;
    while let Some(frame) = timed(&mut stats.decode_wait, || source.next_frame())? {
        check_frame(&frame, decoded, frame_count, size)?;
        decoded += 1;
        if samples.contains(frame.index) {
            let picture = frame
                .picture()
                .ok_or("A decoded frame does not hold its picture")?;
            let screened = repeats.needs_screen(&picture);
            let converted =
                screened.then(|| timed(&mut stats.convert, || padded(&picture, &colour)));
            gathering.push_sample(frame, converted);
            if gathering.full(screening.batch) {
                let (held, pictures) = gathering.close();
                screening.close_group(held, pictures, &mut stats)?;
            }
        } else {
            gathering.push_gap(frame);
        }
        screening.apply_ready(&mut stats)?;
        while screening.waiting.len() >= most_waiting {
            screening.wait_front(&mut stats)?;
        }
        if decoded.is_multiple_of(PROGRESS_FRAMES) || decoded == frame_count {
            progress(decoded as usize, 0);
        }
        debug_assert!(gathering.frames() <= (2 * screening.batch + 1) * step as usize);
    }
    timed(&mut stats.decode_wait, || source.finish())?;
    if decoded != frame_count {
        return Err("The decoded frames ended before the video's final frame".into());
    }
    let (held, pictures) = gathering.finish()?;
    screening.close_group(held, pictures, &mut stats)?;
    while !screening.waiting.is_empty() {
        screening.wait_front(&mut stats)?;
    }
    stats.frames_decoded = decoded;
    screening.tracker.document.decoded_frames = decoded;
    let Screening {
        flight, tracker, ..
    } = screening;
    stats.keyframes_held_peak_bytes = tracker.keyframes_peak_bytes() as u64;
    let pool = flight.into_pool();
    let document = confirm_keyframes(
        tracker.close(limits.min_confirm_frames, limits.min_confirm_confidence),
        &colour,
        source,
        pool,
        root,
        &mut stats,
        progress,
    )?;
    stats.warmup_s = pool.warmup_s();
    stats.engine_build_s = pool.engine_build_s();
    stats.notes.extend(pool.notes());
    Ok((document, stats))
}

/// A decoded frame must be the next one of the timeline, at the video's size, holding its
/// picture.
fn check_frame(frame: &YuvFrame, decoded: u64, count: u64, size: (u32, u32)) -> TextResult<()> {
    if frame.index != decoded
        || frame.index >= count
        || (frame.width, frame.height) != size
        || frame.picture().is_none()
    {
        return Err("The decoded frames do not match the video's frame timeline".into());
    }
    Ok(())
}

/// The screening side of the scan: jobs in flight, the groups waiting for them, and the tracker
/// that observes the groups in order.
struct Screening<'a, 'p> {
    flight: Flight<'p>,
    tracker: Tracker<'a>,
    colour: Coefficients,
    batch: usize,
    waiting: VecDeque<Group>,
    prior_sample: Option<HeldSample>,
}

impl Screening<'_, '_> {
    /// Submits a closed group's pictures as one screening job and queues the group.
    fn close_group(
        &mut self,
        samples: Vec<HeldSample>,
        pictures: Vec<PaddedFrame>,
        stats: &mut ScanStats,
    ) -> TextResult<()> {
        if samples.is_empty() {
            return Ok(());
        }
        let job = if pictures.is_empty() {
            None
        } else {
            stats.frames_screened += pictures.len() as u64;
            Some(self.flight.submit(Priority::Screen, pictures)?)
        };
        self.waiting.push_back(Group {
            prior: None,
            samples,
            job,
        });
        Ok(())
    }

    /// Observes every waiting group at the front whose result has arrived.
    fn apply_ready(&mut self, stats: &mut ScanStats) -> TextResult<()> {
        while self
            .waiting
            .front()
            .is_some_and(|group| group.job.is_none_or(|seq| self.flight.answered(seq)))
        {
            let group = self
                .waiting
                .pop_front()
                .ok_or("A waiting group went missing")?;
            self.observe(group, stats)?;
        }
        Ok(())
    }

    /// Waits for the front group's result, then observes every group that is ready.
    fn wait_front(&mut self, stats: &mut ScanStats) -> TextResult<()> {
        let pending = self
            .waiting
            .front()
            .and_then(|group| group.job)
            .filter(|&seq| !self.flight.answered(seq));
        if pending.is_some() {
            timed(&mut stats.screen, || self.flight.receive())?;
        }
        self.apply_ready(stats)
    }

    /// Observes a group's samples in order, then bisects its transitions through probes that
    /// run ahead of the waiting screening jobs.
    fn observe(&mut self, mut group: Group, stats: &mut ScanStats) -> TextResult<()> {
        group.prior = self.prior_sample.take();
        let mut screens = match group.job {
            Some(seq) => self
                .flight
                .take(seq)
                .ok_or("A screening result went missing")?,
            None => Vec::new(),
        }
        .into_iter();
        let mut transitions: Vec<Transition> = Vec::new();
        for sample in &group.samples {
            let screened = if sample.screened {
                Some(screens.next().ok_or("A screened sample has no result")?)
            } else {
                None
            };
            self.tracker
                .observe(sample, screened, &mut transitions, stats)?;
        }
        self.tracker.forget_unqualified_entries();
        let (flight, batch) = (&mut self.flight, self.batch);
        let mut screen_probes = |pictures: Vec<PaddedFrame>| -> TextResult<Vec<Regions>> {
            let mut jobs = Vec::new();
            let mut pictures = pictures.into_iter().peekable();
            while pictures.peek().is_some() {
                let chunk: Vec<PaddedFrame> = pictures.by_ref().take(batch).collect();
                jobs.push(flight.submit(Priority::Probe, chunk)?);
            }
            let mut regions = Vec::new();
            for seq in jobs {
                regions.extend(flight.wait(seq)?);
            }
            Ok(regions)
        };
        let answers = narrow(
            &transitions,
            &group,
            &self.colour,
            &mut screen_probes,
            stats,
        )?;
        self.tracker.settle(&group, &transitions, &answers)?;
        self.prior_sample = group.samples.pop();
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/scan.rs"]
mod tests;
