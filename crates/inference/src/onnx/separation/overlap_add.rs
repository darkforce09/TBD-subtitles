//! Streaming a long stereo track through a fixed-window separation model with weighted
//! overlap-add.
//!
//! **Role:** cut the incoming stereo samples into model windows at a fixed step, run the model on
//! batches of windows, add each window's vocals back weighted by the model's window weights, and
//! hand out every sample once no later window can still change it, together with the mix sample
//! it came from.
//!
//! **Position:** used by `stages::separation`; drives any [`WindowModel`] (`MdxNet`, `MelRoformer`).
//!
//! **Signals and state:** holds the samples of the windows not yet finished and their weighted
//! sums, a few windows long, never the whole track.
//!
//! **Invariants:** every input sample comes out exactly once, in order, as a (mix, vocals) pair;
//! a sample's vocals are the weight-normalised sum of every window that covered it.

use super::super::OnnxError;

/// Interleaved stereo.
pub const CHANNELS: usize = 2;

/// A separation model that turns fixed-length stereo windows into vocals.
pub trait WindowModel {
    /// Frames per window, the step between windows, the zero frames placed before the track, and
    /// each window frame's weight in the overlap-add.
    fn layout(&self) -> Layout;
    /// Windows per model call.
    fn batch(&self) -> usize;
    /// The vocals of each window (interleaved stereo, the window's length).
    fn separate(&mut self, windows: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, OnnxError>;
}

/// How windows tile the track.
#[derive(Debug, Clone)]
pub struct Layout {
    pub window: usize,
    pub step: usize,
    pub lead: usize,
    pub weights: Vec<f32>,
}

/// Mix and vocals for a run of frames, both interleaved stereo.
#[derive(Debug, Default, Clone)]
pub struct Separated {
    pub mix: Vec<f32>,
    pub vocals: Vec<f32>,
}

/// The streaming driver.
pub struct OverlapAdd<M: WindowModel> {
    model: M,
    layout: Layout,
    /// Padded samples from frame `buffer_start` on.
    buffer: Vec<f32>,
    buffer_start: usize,
    /// Weighted vocal sums and weight sums from frame `out_start` on.
    sums: Vec<f32>,
    weights: Vec<f32>,
    out_start: usize,
    /// The start frame of the next window to run.
    next_window: usize,
    queued: Vec<(usize, Vec<f32>)>,
    /// Frames of the track pushed so far.
    pushed: usize,
}

impl<M: WindowModel> OverlapAdd<M> {
    pub fn new(model: M) -> OverlapAdd<M> {
        let layout = model.layout();
        OverlapAdd {
            buffer: vec![0.0; layout.lead * CHANNELS],
            model,
            layout,
            buffer_start: 0,
            sums: Vec::new(),
            weights: Vec::new(),
            out_start: 0,
            next_window: 0,
            queued: Vec::new(),
            pushed: 0,
        }
    }

    /// The model, for its own counters.
    pub fn model(&self) -> &M {
        &self.model
    }

    /// Add interleaved stereo samples; returns the frames that are now final.
    pub fn push(&mut self, stereo: &[f32]) -> Result<Separated, OnnxError> {
        self.buffer.extend_from_slice(stereo);
        self.pushed += stereo.len() / CHANNELS;
        self.run_ready(false)?;
        Ok(self.emit(self.settled().min(self.layout.lead + self.pushed)))
    }

    /// Pad the end with silence, run the last windows, and return the remaining frames with the
    /// model, whose counters the caller may read.
    pub fn finish(mut self) -> Result<(Separated, M), OnnxError> {
        let end = self.layout.lead + self.pushed;
        while self.next_window < end {
            let needed = (self.next_window + self.layout.window).saturating_sub(self.buffer_end());
            self.buffer
                .extend(std::iter::repeat_n(0.0, needed * CHANNELS));
            self.run_ready(true)?;
        }
        self.run_ready(true)?;
        let rest = self.emit(end);
        Ok((rest, self.model))
    }

    /// Frames before this one are covered by every window that will ever cover them.
    fn settled(&self) -> usize {
        self.queued
            .first()
            .map(|(start, _)| *start)
            .unwrap_or(self.next_window)
    }

    fn buffer_end(&self) -> usize {
        self.buffer_start + self.buffer.len() / CHANNELS
    }

    /// Queue every window the buffer covers; run full batches, or everything when `flush`.
    fn run_ready(&mut self, flush: bool) -> Result<(), OnnxError> {
        let end = self.layout.lead + self.pushed;
        while self.next_window + self.layout.window <= self.buffer_end() && self.next_window < end {
            let from = (self.next_window - self.buffer_start) * CHANNELS;
            let window = self.buffer[from..from + self.layout.window * CHANNELS].to_vec();
            self.queued.push((self.next_window, window));
            self.next_window += self.layout.step;
            if self.queued.len() >= self.model.batch() {
                self.run_queue()?;
            }
        }
        if flush && !self.queued.is_empty() {
            self.run_queue()?;
        }
        Ok(())
    }

    fn run_queue(&mut self) -> Result<(), OnnxError> {
        let queued = std::mem::take(&mut self.queued);
        let windows: Vec<Vec<f32>> = queued.iter().map(|(_, w)| w.clone()).collect();
        let vocals = self.model.separate(&windows)?;
        for ((start, _), vocals) in queued.iter().zip(vocals) {
            let reach = start + self.layout.window - self.out_start;
            if self.weights.len() < reach {
                self.weights.resize(reach, 0.0);
                self.sums.resize(reach * CHANNELS, 0.0);
            }
            for (i, weight) in self.layout.weights.iter().enumerate() {
                let at = start + i - self.out_start;
                self.weights[at] += weight;
                for c in 0..CHANNELS {
                    self.sums[at * CHANNELS + c] += vocals[i * CHANNELS + c] * weight;
                }
            }
        }
        Ok(())
    }

    /// Hand out padded frames `[out_start, until)` that belong to the track, and drop them.
    fn emit(&mut self, until: usize) -> Separated {
        let mut out = Separated::default();
        let until = until.max(self.out_start);
        for frame in self.out_start..until {
            if frame < self.layout.lead || frame >= self.layout.lead + self.pushed {
                continue;
            }
            let at = frame - self.out_start;
            let weight = self.weights.get(at).copied().unwrap_or(0.0);
            let from = (frame - self.buffer_start) * CHANNELS;
            for c in 0..CHANNELS {
                out.mix.push(self.buffer[from + c]);
                let sum = self.sums.get(at * CHANNELS + c).copied().unwrap_or(0.0);
                out.vocals
                    .push(if weight > 1e-6 { sum / weight } else { 0.0 });
            }
        }
        let done = until - self.out_start;
        let done = done.min(self.weights.len());
        self.weights.drain(..done);
        self.sums.drain(..done * CHANNELS);
        self.out_start += until - self.out_start;
        // Keep the buffer from the next window or the next frame to emit, whichever is first.
        let keep_from = self.next_window.min(self.out_start).max(self.buffer_start);
        let drop = (keep_from - self.buffer_start) * CHANNELS;
        self.buffer.drain(..drop.min(self.buffer.len()));
        self.buffer_start = keep_from;
        out
    }
}

#[cfg(test)]
#[path = "tests/overlap_add.rs"]
mod tests;
