use std::cell::RefCell;

use image::{Rgb, RgbImage};
use inference::ocr::{OcrError, TextDetection};
use job_model::onscreen::Quad;
use job_model::outputs::{ShotChanges, VideoStream};

use super::*;

const PROXY: (u32, u32) = (32, 18);

/// A plain grey video of `count` proxy frames at 24 fps.
struct Plain {
    timeline: Vec<(f64, f64)>,
    next: u64,
}

impl Plain {
    fn new(count: u64) -> Self {
        Self {
            timeline: (0..count)
                .map(|i| (i as f64 / 24.0, (i + 1) as f64 / 24.0))
                .collect(),
            next: 0,
        }
    }
}

impl FrameSource for Plain {
    fn proxy_size(&self) -> (u32, u32) {
        PROXY
    }

    fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    fn next_proxy(&mut self) -> TextResult<Option<ProxyFrame>> {
        let Some(&(time_s, end_s)) = self.timeline.get(self.next as usize) else {
            return Ok(None);
        };
        let index = self.next;
        self.next += 1;
        Ok(Some(ProxyFrame {
            index,
            time_s,
            end_s,
            rgb: RgbImage::from_pixel(PROXY.0, PROXY.1, Rgb([90, 90, 90])),
        }))
    }

    fn finish_proxies(&mut self) -> TextResult<()> {
        Ok(())
    }

    fn stills(&mut self, indices: &[u64]) -> TextResult<Vec<RgbImage>> {
        Ok(indices
            .iter()
            .map(|_| RgbImage::from_pixel(64, 36, Rgb([90, 90, 90])))
            .collect())
    }
}

/// A detector that finds nothing and counts what it was shown.
#[derive(Default)]
struct Blind {
    screened: usize,
    detected: usize,
}

impl TextDetection for Blind {
    fn screen_batch(&mut self, images: &[RgbImage]) -> Result<Vec<Vec<(Quad, f64)>>, OcrError> {
        self.screened += images.len();
        Ok(vec![Vec::new(); images.len()])
    }

    fn detect(&mut self, _image: &RgbImage) -> Result<Vec<(Quad, f64)>, OcrError> {
        self.detected += 1;
        Ok(Vec::new())
    }
}

fn stream() -> VideoStream {
    VideoStream {
        index: 0,
        codec: "h264".into(),
        width: 64,
        height: 36,
        frame_rate_num: 24,
        frame_rate_den: 1,
        start_time_s: 0.0,
        ..Default::default()
    }
}

fn root(name: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("tbd-detect-timing-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn a_measured_scan_returns_the_plain_scan_s_document_and_counts_its_frames() {
    let cuts = ShotChanges::default();
    let folder = root("same");
    let (mut plain_source, mut plain_detector) = (Plain::new(50), Blind::default());
    let plain = scan(
        &mut plain_source,
        &stream(),
        &cuts,
        &folder,
        &mut plain_detector,
        &|_, _| {},
    )
    .unwrap();
    let (mut source, mut detector) = (Plain::new(50), Blind::default());
    let (document, stats) = scan_measured(
        &mut source,
        &stream(),
        &cuts,
        &folder,
        &mut detector,
        &|_, _| {},
    )
    .unwrap();
    let _ = std::fs::remove_dir_all(&folder);
    assert_eq!(document, plain);
    assert_eq!(stats.frames_decoded, 50);
    assert_eq!(stats.frames_screened, detector.screened as u64);
    assert_eq!(detector.screened, plain_detector.screened);
}

#[test]
fn the_timed_detector_counts_screens_and_times_confirmations_apart() {
    let stats = RefCell::new(ScanStats::default());
    let mut inner = Blind::default();
    let mut timed = TimedDetector {
        inner: &mut inner,
        stats: &stats,
    };
    let image = RgbImage::new(4, 4);
    timed
        .screen_batch(&[image.clone(), image.clone(), image.clone()])
        .unwrap();
    timed.detect(&image).unwrap();
    assert_eq!(stats.borrow().frames_screened, 3);
    assert_eq!(inner.detected, 1);
}

#[test]
fn the_timed_source_counts_only_the_frames_it_hands_on() {
    let stats = RefCell::new(ScanStats::default());
    let mut inner = Plain::new(3);
    let mut timed = TimedSource {
        inner: &mut inner,
        stats: &stats,
    };
    while timed.next_proxy().unwrap().is_some() {}
    timed.finish_proxies().unwrap();
    assert_eq!(timed.stills(&[0, 1]).unwrap().len(), 2);
    assert_eq!(stats.borrow().frames_decoded, 3);
}
