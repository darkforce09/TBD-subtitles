//! The screened samples kept in memory because they may still become an occurrence's keyframe.
//!
//! **Role:** keep, for every occurrence, the samples that can still be the one nearest its
//! middle, keep only the chosen one once it ends, and bound everything held by a byte budget so
//! confirmation reads most keyframes from memory instead of seeking the video again.
//! **Position:** fed by the coordinator as it observes samples in order; read by confirmation
//! after the scan.
//! **Signals and state:** one window of `(frame index, time)` entries per occurrence, the shared
//! frames with how many windows hold each, and the bytes they take.
//! **Invariants:** a frame is held while at least one window holds it, once; while an occurrence
//! is active its window keeps every sample from the last one at or before the middle of its
//! start and the latest sample it was seen on, so the sample nearest its final middle is never
//! released; the held bytes never stay above the budget: past it, the active window holding the
//! most samples gives up its frames and falls back to a still decoded from the video, while
//! chosen keyframes stay; every decision follows the order samples are observed, never the order
//! the detector answers in.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use media_io::yuv::YuvFrame;

/// The bytes the keyframe candidates may hold within the 24 GB workstation budget:
/// about 5,500 frames at 1920 by 1080.
pub(crate) const CANDIDATE_BUDGET: usize = 16 << 30;

/// Where an occurrence's keyframe will come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    /// Active: its window still collects samples.
    Open,
    /// Ended with this frame held as its keyframe.
    Chosen(u64),
    /// Its keyframe is decoded from the video after the scan.
    Fallback,
}

struct Window {
    entries: VecDeque<(u64, f64)>,
    state: State,
}

/// Every occurrence's keyframe candidates and the frames they share.
pub(crate) struct Candidates {
    budget: usize,
    bytes: usize,
    peak: usize,
    held: BTreeMap<u64, (Arc<YuvFrame>, usize)>,
    windows: Vec<Window>,
}

impl Candidates {
    pub(crate) fn new(budget: usize) -> Self {
        Self {
            budget,
            bytes: 0,
            peak: 0,
            held: BTreeMap::new(),
            windows: Vec::new(),
        }
    }

    /// The most bytes held at once after keeping within the budget.
    pub(crate) fn peak_bytes(&self) -> usize {
        self.peak
    }

    /// Adds `sample` to `occurrence`'s window, then keeps within the budget.
    pub(crate) fn offer(&mut self, occurrence: usize, sample: &Arc<YuvFrame>) {
        while self.windows.len() <= occurrence {
            self.windows.push(Window {
                entries: VecDeque::new(),
                state: State::Open,
            });
        }
        if self.windows[occurrence].state != State::Open {
            return;
        }
        self.windows[occurrence]
            .entries
            .push_back((sample.index, sample.time_s));
        let entry = self
            .held
            .entry(sample.index)
            .or_insert_with(|| (Arc::clone(sample), 0));
        if entry.1 == 0 {
            self.bytes += entry.0.data.len();
        }
        entry.1 += 1;
        self.enforce_budget();
        self.peak = self.peak.max(self.bytes);
    }

    /// Releases the samples of `occurrence`'s window that can no longer be nearest its middle,
    /// which lies at or after `floor_s`: every sample before the last one at or before it.
    pub(crate) fn prune(&mut self, occurrence: usize, floor_s: f64) {
        let Some(window) = self.windows.get_mut(occurrence) else {
            return;
        };
        if window.state != State::Open {
            return;
        }
        let keep_from = window
            .entries
            .iter()
            .rposition(|&(_, time_s)| time_s <= floor_s)
            .unwrap_or(0);
        let released: Vec<u64> = window
            .entries
            .drain(..keep_from)
            .map(|(index, _)| index)
            .collect();
        for index in released {
            self.release(index);
        }
    }

    /// Ends `occurrence`'s window with `keyframe` chosen: the frame stays held when its window
    /// holds it, and every other sample is released; otherwise the occurrence falls back.
    pub(crate) fn choose(&mut self, occurrence: usize, keyframe: u64) {
        let Some(window) = self.windows.get_mut(occurrence) else {
            return;
        };
        if window.state != State::Open {
            return;
        }
        let entries = std::mem::take(&mut window.entries);
        let mut kept = None;
        for (index, time_s) in entries {
            if index == keyframe && kept.is_none() {
                // The chosen sample keeps the one hold its window has on it.
                kept = Some((index, time_s));
            } else {
                self.release(index);
            }
        }
        let window = &mut self.windows[occurrence];
        window.state = kept.map_or(State::Fallback, |_| State::Chosen(keyframe));
        window.entries.extend(kept);
    }

    /// The held frames among `wanted`, by index; everything else is released.
    pub(crate) fn into_frames(self, wanted: &[u64]) -> BTreeMap<u64, Arc<YuvFrame>> {
        let mut held = self.held;
        wanted
            .iter()
            .filter_map(|index| held.remove(index).map(|(frame, _)| (*index, frame)))
            .collect()
    }

    fn release(&mut self, index: u64) {
        let Some(entry) = self.held.get_mut(&index) else {
            return;
        };
        entry.1 -= 1;
        if entry.1 == 0 {
            self.bytes -= entry.0.data.len();
            self.held.remove(&index);
        }
    }

    /// Gives up frames until the held bytes fit the budget: the active window holding the most
    /// samples falls back, the earliest occurrence on a tie. Kept keyframes never need to go,
    /// since choosing and pruning only ever release frames.
    fn enforce_budget(&mut self) {
        while self.bytes > self.budget {
            let Some(victim) = (0..self.windows.len())
                .filter(|&o| self.windows[o].state == State::Open)
                .filter(|&o| !self.windows[o].entries.is_empty())
                .max_by_key(|&o| (self.windows[o].entries.len(), std::cmp::Reverse(o)))
            else {
                return;
            };
            let window = &mut self.windows[victim];
            window.state = State::Fallback;
            let entries = std::mem::take(&mut window.entries);
            for (index, _) in entries {
                self.release(index);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/candidates.rs"]
mod tests;
