//! Coarse sampling, repeat screening and lockstep bisection for the visual scan.
//!
//! **Role:** choose the proxy frames the detector screens, recognise a sample that repeats the
//! last screened picture, hold samples with the frames between them, and narrow every entry or
//! exit of a region to its exact frame.
//! **Position:** helpers of `detect::scan`; no decoding, no model and no file output.
//! **Signals and state:** the sample schedule, a pending batch of at most `SCREEN_BATCH` samples
//! with their gaps, and interval searches.
//! **Invariants:** every `step`-th frame, both frames around every shot cut and the final frame
//! are samples, so a gap never crosses a cut and holds at most `step - 1` frames; a search needs
//! at most ceil(log2(step)) probes; each bisection step screens its distinct probes in one call.

use image::RgbImage;
use job_model::outputs::ShotChanges;

use super::source::ProxyFrame;
use crate::onscreen_text::TextResult;

/// Samples screened in one detector call.
pub(super) const SCREEN_BATCH: usize = 8;
/// The side of a repeat-comparison block, in proxy pixels.
const BLOCK: u32 = 32;
/// The mean absolute difference per channel above which a block has changed.
const BLOCK_MEAN: u64 = 4;

/// Frames between coarse samples: about half a second, at least one.
pub(super) fn sample_step(fps: f64) -> u64 {
    (fps / 2.0).round().max(1.0) as u64
}

/// Which frame indices the detector screens.
pub(super) struct Samples {
    step: u64,
    count: u64,
    /// The first frame at or after every cut and the frame before it, ascending.
    boundaries: Vec<u64>,
}

impl Samples {
    pub(super) fn new(timeline: &[(f64, f64)], cuts: &ShotChanges, step: u64) -> Self {
        let mut boundaries = Vec::with_capacity(cuts.cuts.len() * 2);
        for cut in &cuts.cuts {
            let first = timeline.partition_point(|&(time_s, _)| time_s < cut.time_s) as u64;
            if first < timeline.len() as u64 {
                boundaries.push(first);
            }
            if first > 0 {
                boundaries.push(first - 1);
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        Self {
            step: step.max(1),
            count: timeline.len() as u64,
            boundaries,
        }
    }

    pub(super) fn contains(&self, index: u64) -> bool {
        index.is_multiple_of(self.step)
            || index + 1 == self.count
            || self.boundaries.binary_search(&index).is_ok()
    }
}

/// Whether two proxies of one size differ by a mean of at most 4 per channel in every 32 by 32
/// block, so the later one can reuse the earlier one's screen.
pub(super) fn near_duplicate(a: &RgbImage, b: &RgbImage) -> bool {
    if a.dimensions() != b.dimensions() {
        return false;
    }
    let (width, height) = a.dimensions();
    let row = width as usize * 3;
    for top in (0..height).step_by(BLOCK as usize) {
        for left in (0..width).step_by(BLOCK as usize) {
            let across = (left + BLOCK).min(width) - left;
            let down = (top + BLOCK).min(height) - top;
            let mut total = 0u64;
            for y in top..top + down {
                let start = y as usize * row + left as usize * 3;
                let end = start + across as usize * 3;
                total += a.as_raw()[start..end]
                    .iter()
                    .zip(&b.as_raw()[start..end])
                    .map(|(p, q)| u64::from(p.abs_diff(*q)))
                    .sum::<u64>();
            }
            if total > BLOCK_MEAN * u64::from(across) * u64::from(down) * 3 {
                return false;
            }
        }
    }
    true
}

/// A sample with the frames since the previous sample.
pub(super) struct PendingSample {
    pub(super) frame: ProxyFrame,
    pub(super) gap: Vec<ProxyFrame>,
}

impl PendingSample {
    /// The frame at `index` among this sample and its gap.
    pub(super) fn frame(&self, index: u64) -> Option<&ProxyFrame> {
        if index == self.frame.index {
            return Some(&self.frame);
        }
        let first = self.gap.first()?.index;
        self.gap
            .get(usize::try_from(index.checked_sub(first)?).ok()?)
            .filter(|frame| frame.index == index)
    }

    /// The previous sample's index: a change seen at this sample lies after it.
    pub(super) fn previous_sample(&self) -> u64 {
        self.gap
            .first()
            .map_or(self.frame.index, |first| first.index)
            .saturating_sub(1)
    }
}

/// Samples waiting for one screening call, and the gap after the last of them.
#[derive(Default)]
pub(super) struct Pending {
    samples: Vec<PendingSample>,
    gap: Vec<ProxyFrame>,
}

impl Pending {
    /// Adds the next frame; hands out the batch once it holds `SCREEN_BATCH` samples.
    pub(super) fn push(&mut self, frame: ProxyFrame, sample: bool) -> Option<Vec<PendingSample>> {
        if !sample {
            self.gap.push(frame);
            return None;
        }
        self.samples.push(PendingSample {
            frame,
            gap: std::mem::take(&mut self.gap),
        });
        (self.samples.len() == SCREEN_BATCH).then(|| std::mem::take(&mut self.samples))
    }

    /// Every frame held: the samples, their gaps and the gap after the last sample.
    pub(super) fn frames(&self) -> usize {
        self.gap.len()
            + self
                .samples
                .iter()
                .map(|sample| 1 + sample.gap.len())
                .sum::<usize>()
    }

    /// The last, partial batch; frames after its final sample mean the stream stopped short.
    pub(super) fn finish(self) -> TextResult<Vec<PendingSample>> {
        if !self.gap.is_empty() {
            return Err("The screening copy ended between samples".into());
        }
        Ok(self.samples)
    }
}

/// The kind of change a search looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Seek {
    /// The first frame showing the region.
    Entry,
    /// The first frame no longer showing it.
    Exit,
}

/// A change known to lie in (`lo`, `hi`]; once no frame lies between them, `hi` is the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Search {
    pub(super) lo: u64,
    pub(super) hi: u64,
    pub(super) seek: Seek,
}

impl Search {
    pub(super) fn new(lo: u64, hi: u64, seek: Seek) -> Self {
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
/// indices in one call, then `present(cache, search, index)` answers each search's probe.
pub(super) fn bisect<C>(
    searches: &mut [Search],
    cache: &mut C,
    mut screen: impl FnMut(&mut C, &[u64]) -> TextResult<()>,
    present: impl Fn(&C, usize, u64) -> bool,
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
        for (index, search) in searches.iter_mut().enumerate() {
            if search.open() {
                let shown = present(cache, index, search.probe());
                search.narrow(shown);
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/screen.rs"]
mod tests;
