//! Where a scan spends its time: the frame source and the detector wrapped in timers that count
//! what passes through them, around an unchanged scan.
//!
//! **Role:** run `scan` over a source and a detector that time every proxy read, every screening
//! call, every full-resolution confirmation and every batch of stills, and count the proxies
//! decoded and the pictures screened.
//!
//! **Position:** called by `pipeline::tasks::onscreen` for the `text_detect` step, in place of
//! `scan`; the stats become the step's notes.
//!
//! **Signals and state:** one `ScanStats` the two wrappers add to while the scan runs.
//!
//! **Invariants:** the wrappers forward every call and its result unchanged, so the document is
//! exactly the one `scan` returns; screening covers the sample screens and the bisection probes
//! alike.

use std::cell::RefCell;
use std::path::Path;
use std::time::{Duration, Instant};

use image::RgbImage;
use inference::ocr::{OcrError, TextDetection};
use job_model::onscreen::{Quad, TextDocument};
use job_model::outputs::{ShotChanges, VideoStream};

use super::scan;
use super::source::{FrameSource, ProxyFrame};
use crate::onscreen_text::TextResult;

/// Where one scan spent its time and what it handled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanStats {
    /// Waiting on the proxy decoder for the next frame.
    pub decode_wait: Duration,
    /// In the screening detector: the samples and the bisection probes.
    pub detect: Duration,
    /// In the server detector, confirming each keyframe on its full-resolution still.
    pub confirm: Duration,
    /// Decoding the full-resolution stills.
    pub stills: Duration,
    /// The proxy frames decoded.
    pub frames_decoded: u64,
    /// The pictures sent to the screening detector.
    pub frames_screened: u64,
}

/// `scan`, with where its time went.
pub fn scan_measured(
    source: &mut dyn FrameSource,
    stream: &VideoStream,
    cuts: &ShotChanges,
    root: &Path,
    detector: &mut dyn TextDetection,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<(TextDocument, ScanStats)> {
    let stats = RefCell::new(ScanStats::default());
    let mut timed_source = TimedSource {
        inner: source,
        stats: &stats,
    };
    let mut timed_detector = TimedDetector {
        inner: detector,
        stats: &stats,
    };
    let document = scan(
        &mut timed_source,
        stream,
        cuts,
        root,
        &mut timed_detector,
        progress,
    )?;
    Ok((document, stats.into_inner()))
}

/// `f`'s result and how long it took.
fn timed<T>(f: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let result = f();
    (result, started.elapsed())
}

struct TimedSource<'a> {
    inner: &'a mut dyn FrameSource,
    stats: &'a RefCell<ScanStats>,
}

impl FrameSource for TimedSource<'_> {
    fn proxy_size(&self) -> (u32, u32) {
        self.inner.proxy_size()
    }

    fn timeline(&self) -> &[(f64, f64)] {
        self.inner.timeline()
    }

    fn next_proxy(&mut self) -> TextResult<Option<ProxyFrame>> {
        let (frame, spent) = timed(|| self.inner.next_proxy());
        let mut stats = self.stats.borrow_mut();
        stats.decode_wait += spent;
        if matches!(frame, Ok(Some(_))) {
            stats.frames_decoded += 1;
        }
        frame
    }

    fn finish_proxies(&mut self) -> TextResult<()> {
        let (finished, spent) = timed(|| self.inner.finish_proxies());
        self.stats.borrow_mut().decode_wait += spent;
        finished
    }

    fn stills(&mut self, indices: &[u64]) -> TextResult<Vec<RgbImage>> {
        let (stills, spent) = timed(|| self.inner.stills(indices));
        self.stats.borrow_mut().stills += spent;
        stills
    }
}

struct TimedDetector<'a> {
    inner: &'a mut dyn TextDetection,
    stats: &'a RefCell<ScanStats>,
}

impl TextDetection for TimedDetector<'_> {
    fn screen_batch(&mut self, images: &[RgbImage]) -> Result<Vec<Vec<(Quad, f64)>>, OcrError> {
        let (screened, spent) = timed(|| self.inner.screen_batch(images));
        let mut stats = self.stats.borrow_mut();
        stats.detect += spent;
        stats.frames_screened += images.len() as u64;
        screened
    }

    fn detect(&mut self, image: &RgbImage) -> Result<Vec<(Quad, f64)>, OcrError> {
        let (found, spent) = timed(|| self.inner.detect(image));
        self.stats.borrow_mut().confirm += spent;
        found
    }
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;
