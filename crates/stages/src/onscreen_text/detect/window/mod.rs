//! The decoded frames the scan holds: samples with the frames since the previous sample, grouped
//! as their screening job, and the samples kept as keyframe candidates.
//!
//! **Role:** gather decoded frames into groups that end with a screening job, so the coordinator
//! can observe a group once its job is answered and bisect inside it, then release its gaps.
//! **Position:** between the frame source and the coordinator; the candidates module keeps the
//! samples that may become keyframes.
//! **Signals and state:** the group being gathered: its samples, the frames after its last sample
//! and its padded pictures awaiting submission.
//! **Invariants:** every frame passes through exactly one group; a sample's gap holds the frames
//! since the previous sample, so a transition seen at a sample is bisected inside its own group;
//! a group closes once its padded pictures fill a screening batch or it holds twice a batch of
//! samples, so repeated samples never pile up unbounded; a stream that stops between samples is
//! an error.

mod candidates;

use std::sync::Arc;

use inference::ocr::pool::PaddedFrame;
use media_io::yuv::YuvFrame;

use crate::onscreen_text::TextResult;

pub(crate) use candidates::{CANDIDATE_BUDGET, Candidates};

/// Samples a group may hold, in screening batches, before it closes with a partial job.
const SAMPLES_PER_BATCH: usize = 2;

/// A sample with the frames since the previous sample, all decoded at full resolution.
pub(crate) struct HeldSample {
    pub(crate) frame: Arc<YuvFrame>,
    pub(crate) gap: Vec<YuvFrame>,
    /// Whether the detector screens this sample; a repeat reuses the last screened regions.
    pub(crate) screened: bool,
}

impl HeldSample {
    /// The frame at `index` among this sample and its gap.
    pub(crate) fn frame(&self, index: u64) -> Option<&YuvFrame> {
        if index == self.frame.index {
            return Some(&self.frame);
        }
        let first = self.gap.first()?.index;
        self.gap
            .get(usize::try_from(index.checked_sub(first)?).ok()?)
            .filter(|frame| frame.index == index)
    }

    /// The previous sample's index: a change seen at this sample lies after it.
    pub(crate) fn previous_sample(&self) -> u64 {
        self.gap
            .first()
            .map_or(self.frame.index, |first| first.index)
            .saturating_sub(1)
    }
}

/// Consecutive samples and the screening job that answers their screened ones, if any.
pub(crate) struct Group {
    pub(crate) samples: Vec<HeldSample>,
    /// The job's sequence number; `None` when every sample repeats an earlier screen.
    pub(crate) job: Option<u64>,
}

impl Group {
    /// The frame at `index` among the group's samples and gaps.
    pub(crate) fn frame(&self, index: u64) -> Option<&YuvFrame> {
        self.samples.iter().find_map(|sample| sample.frame(index))
    }
}

/// The group being gathered from the decoded frames.
#[derive(Default)]
pub(crate) struct Gathering {
    samples: Vec<HeldSample>,
    gap: Vec<YuvFrame>,
    pictures: Vec<PaddedFrame>,
}

impl Gathering {
    /// Adds a frame between samples.
    pub(crate) fn push_gap(&mut self, frame: YuvFrame) {
        self.gap.push(frame);
    }

    /// Adds a sample, with its padded picture when the detector screens it.
    pub(crate) fn push_sample(&mut self, frame: YuvFrame, picture: Option<PaddedFrame>) {
        self.samples.push(HeldSample {
            frame: Arc::new(frame),
            gap: std::mem::take(&mut self.gap),
            screened: picture.is_some(),
        });
        self.pictures.extend(picture);
    }

    /// Whether the group should close now: its pictures fill a batch of `batch`, or it holds
    /// twice that many samples.
    pub(crate) fn full(&self, batch: usize) -> bool {
        self.pictures.len() >= batch || self.samples.len() >= SAMPLES_PER_BATCH * batch.max(1)
    }

    /// Every frame held: the samples, their gaps and the gap after the last sample.
    pub(crate) fn frames(&self) -> usize {
        self.gap.len()
            + self
                .samples
                .iter()
                .map(|sample| 1 + sample.gap.len())
                .sum::<usize>()
    }

    /// The gathered samples and their pictures; the frames after the last sample stay for the
    /// next group.
    pub(crate) fn close(&mut self) -> (Vec<HeldSample>, Vec<PaddedFrame>) {
        (
            std::mem::take(&mut self.samples),
            std::mem::take(&mut self.pictures),
        )
    }

    /// The last group; frames after its final sample mean the stream stopped short.
    pub(crate) fn finish(mut self) -> TextResult<(Vec<HeldSample>, Vec<PaddedFrame>)> {
        if !self.gap.is_empty() {
            return Err("The decoded frames ended between samples".into());
        }
        Ok(self.close())
    }
}

#[cfg(test)]
#[path = "tests/window.rs"]
mod tests;
