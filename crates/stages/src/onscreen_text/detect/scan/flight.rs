//! Screening jobs in flight: numbered on submission, answered in any order, handed out by number.
//!
//! **Role:** submit screening and probe jobs to the detector sessions with sequence numbers,
//! receive their results in whatever order the sessions finish, and keep each until the scan
//! asks for it by number.
//! **Position:** owned by the coordinator; the only part of the scan that talks to the sessions
//! while screening.
//! **Signals and state:** the next sequence number, the frame count of every job not yet
//! received, and the results received but not yet taken.
//! **Invariants:** every job gets a distinct number, increasing in submission order; a result is
//! accepted only for a job in flight and only with one region list per frame; an empty job is
//! never submitted.

use std::collections::HashMap;

use inference::ocr::pool::{PaddedFrame, Priority, ScreenJob, TextScreening};
use job_model::onscreen::Quad;

use crate::onscreen_text::TextResult;

/// One frame's screened regions.
pub(crate) type Regions = Vec<(Quad, f64)>;

/// The jobs submitted to the sessions and the results not yet taken.
pub(crate) struct Flight<'a> {
    pool: &'a mut dyn TextScreening,
    next: u64,
    waiting: HashMap<u64, usize>,
    answered: HashMap<u64, Vec<Regions>>,
}

impl<'a> Flight<'a> {
    pub(crate) fn new(pool: &'a mut dyn TextScreening) -> Self {
        Self {
            pool,
            next: 0,
            waiting: HashMap::new(),
            answered: HashMap::new(),
        }
    }

    /// Submits `frames` as one job at `priority`; its sequence number.
    pub(crate) fn submit(
        &mut self,
        priority: Priority,
        frames: Vec<PaddedFrame>,
    ) -> TextResult<u64> {
        if frames.is_empty() {
            return Err("A screening job needs at least one frame".into());
        }
        let seq = self.next;
        self.next += 1;
        self.waiting.insert(seq, frames.len());
        self.pool.submit(ScreenJob {
            seq,
            priority,
            frames,
        })?;
        Ok(seq)
    }

    /// Whether job `seq`'s result has arrived.
    pub(crate) fn answered(&self, seq: u64) -> bool {
        self.answered.contains_key(&seq)
    }

    /// Takes job `seq`'s result once it has arrived.
    pub(crate) fn take(&mut self, seq: u64) -> Option<Vec<Regions>> {
        self.answered.remove(&seq)
    }

    /// Blocks until the next result arrives and keeps it.
    pub(crate) fn receive(&mut self) -> TextResult<()> {
        if self.waiting.is_empty() {
            return Err("The scan waited for a screening result with no job in flight".into());
        }
        let result = self.pool.recv()?;
        let Some(frames) = self.waiting.remove(&result.seq) else {
            return Err("The text detector answered a job that was not in flight".into());
        };
        if result.regions.len() != frames {
            return Err("The text detector returned a different number of screens".into());
        }
        self.answered.insert(result.seq, result.regions);
        Ok(())
    }

    /// Blocks until job `seq` is answered, keeping the results that arrive before it, and takes
    /// its result.
    pub(crate) fn wait(&mut self, seq: u64) -> TextResult<Vec<Regions>> {
        while !self.answered(seq) {
            self.receive()?;
        }
        self.take(seq)
            .ok_or_else(|| "A screening result went missing".into())
    }

    /// The sessions, once screening is over.
    pub(crate) fn into_pool(self) -> &'a mut dyn TextScreening {
        self.pool
    }
}

#[cfg(test)]
#[path = "tests/flight.rs"]
mod tests;
