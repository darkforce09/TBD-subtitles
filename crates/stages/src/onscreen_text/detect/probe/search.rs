//! Lockstep bisection of the frames between two samples.
//!
//! **Role:** narrow every entry or exit of a region to its exact frame, all searches of a group
//! advancing together, one screening call per step.
//! **Position:** used by the probes module; no decoding, no model and no file output.
//! **Signals and state:** interval searches and the caller's probe cache.
//! **Invariants:** a search needs at most ceil(log2(step)) probes; each step screens its
//! distinct probes in one call; every open search's probe is answered from that step's cache,
//! the answers computed side by side and applied in search order.

use rayon::prelude::*;

use crate::onscreen_text::TextResult;

/// The kind of change a search looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seek {
    /// The first frame showing the region.
    Entry,
    /// The first frame no longer showing it.
    Exit,
}

/// A change known to lie in (`lo`, `hi`]; once no frame lies between them, `hi` is the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Search {
    pub(crate) lo: u64,
    pub(crate) hi: u64,
    pub(crate) seek: Seek,
}

impl Search {
    pub(crate) fn new(lo: u64, hi: u64, seek: Seek) -> Self {
        Self {
            lo: lo.min(hi),
            hi,
            seek,
        }
    }

    fn open(&self) -> bool {
        self.hi - self.lo > 1
    }

    fn probe(&self) -> u64 {
        self.lo + (self.hi - self.lo) / 2
    }

    fn narrow(&mut self, present: bool) {
        let probe = self.probe();
        let changed = match self.seek {
            Seek::Entry => present,
            Seek::Exit => !present,
        };
        if changed {
            self.hi = probe;
        } else {
            self.lo = probe;
        }
    }
}

/// Advances every search one probe per step: `screen` receives the step's distinct probe
/// indices in one call, then `present(cache, search, index)` answers each open search's probe,
/// the searches side by side.
pub(crate) fn bisect<C: Sync>(
    searches: &mut [Search],
    cache: &mut C,
    mut screen: impl FnMut(&mut C, &[u64]) -> TextResult<()>,
    present: impl Fn(&C, usize, u64) -> bool + Sync,
) -> TextResult<()> {
    loop {
        let mut probes: Vec<u64> = searches
            .iter()
            .filter(|search| search.open())
            .map(Search::probe)
            .collect();
        if probes.is_empty() {
            return Ok(());
        }
        probes.sort_unstable();
        probes.dedup();
        screen(cache, &probes)?;
        let shared: &C = cache;
        let answers: Vec<Option<bool>> = searches
            .par_iter()
            .enumerate()
            .map(|(index, search)| {
                search
                    .open()
                    .then(|| present(shared, index, search.probe()))
            })
            .collect();
        for (search, answer) in searches.iter_mut().zip(answers) {
            if let Some(shown) = answer {
                search.narrow(shown);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/search.rs"]
mod tests;
