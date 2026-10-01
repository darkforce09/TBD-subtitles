//! Scripted videos, frame sources and detector sessions shared by the detection tests.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use image::{Rgb, RgbImage};
use inference::ocr::OcrError;
use inference::ocr::pool::{
    ConfirmJob, ConfirmResult, PaddedFrame, Priority, ScreenJob, ScreenResult, ScreenShape,
    TextScreening,
};
use job_model::onscreen::{Point, Quad};
use job_model::outputs::VideoStream;
use media_io::frame_queue::PooledBuffer;
use media_io::yuv::{ChromaLayout, YuvFrame};

use super::FrameSource;
use crate::onscreen_text::TextResult;

/// The scripted videos' picture size.
pub const SIZE: (u32, u32) = (128, 72);
/// The scripted picture's background luma.
pub const BACKGROUND: u8 = 100;
/// The confirming detector's quads sit this far from the screening quads.
pub const EXACT_OFFSET: f64 = 0.5;

/// A folder under the system's temporary folder, removed when dropped.
pub struct Temporary(pub PathBuf);

impl Temporary {
    pub fn new(name: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-detect-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Scripted writing: a rectangle in source pixels, the frames that show it, and its ink on
/// paper. The pattern starts with ink at its top-left pixel, which the scripted detector looks
/// for.
#[derive(Clone, Debug)]
pub struct Writing {
    pub rect: (u32, u32, u32, u32),
    pub frames: RangeInclusive<u64>,
    pub ink: u8,
    pub paper: u8,
}

/// Writing at the usual place, 80 by 20 pixels.
pub fn writing(frames: RangeInclusive<u64>, ink: u8, paper: u8) -> Writing {
    Writing {
        rect: (16, 12, 80, 20),
        frames,
        ink,
        paper,
    }
}

pub fn rectangle(left: f64, top: f64, width: f64, height: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point {
            x: left + width,
            y: top,
        },
        Point {
            x: left + width,
            y: top + height,
        },
        Point {
            x: left,
            y: top + height,
        },
    ])
}

/// The quad a detector reports for `writing`, moved by `offset`.
pub fn region(writing: &Writing, offset: f64) -> Quad {
    let (left, top, width, height) = writing.rect;
    rectangle(
        f64::from(left) + offset,
        f64::from(top) + offset,
        f64::from(width),
        f64::from(height),
    )
}

/// The grey level of every pixel of frame `index`; with `flicker`, a corner patch that is not
/// writing changes on every frame.
pub fn luma(writings: &[Writing], index: u64, flicker: bool) -> Vec<u8> {
    let (width, height) = SIZE;
    let mut plane = vec![BACKGROUND; (width * height) as usize];
    for writing in writings.iter().filter(|w| w.frames.contains(&index)) {
        let (left, top, w, h) = writing.rect;
        for y in 0..h {
            for x in 0..w {
                let inked = (x / 6 + y / 4).is_multiple_of(2);
                let value = if inked { writing.ink } else { writing.paper };
                plane[((top + y) * width + left + x) as usize] = value;
            }
        }
    }
    if flicker {
        let level = (index * 37 % 200) as u8 + 20;
        for y in 60..68 {
            for x in 112..120 {
                plane[(y * width + x) as usize] = level;
            }
        }
    }
    plane
}

/// Frame `index` as full-range yuv420p with neutral chroma: its colour is its grey level.
pub fn yuv_frame(
    writings: &[Writing],
    index: u64,
    flicker: bool,
    timeline: &[(f64, f64)],
) -> YuvFrame {
    let mut data = luma(writings, index, flicker);
    data.resize(data.len() * 3 / 2, 128);
    let (time_s, end_s) = timeline[index as usize];
    YuvFrame {
        index,
        time_s,
        end_s,
        width: SIZE.0,
        height: SIZE.1,
        layout: ChromaLayout::Planar,
        data: PooledBuffer::detached(data),
    }
}

/// Frame `index` as an rgb24 still.
pub fn still(writings: &[Writing], index: u64, flicker: bool) -> RgbImage {
    let plane = luma(writings, index, flicker);
    RgbImage::from_fn(SIZE.0, SIZE.1, |x, y| {
        Rgb([plane[(y * SIZE.0 + x) as usize]; 3])
    })
}

/// `count` frames at 24 fps.
pub fn timeline(count: u64) -> Vec<(f64, f64)> {
    (0..count)
        .map(|i| (i as f64 / 24.0, (i + 1) as f64 / 24.0))
        .collect()
}

/// A full-range 24 fps stream of the scripted size.
pub fn stream() -> VideoStream {
    VideoStream {
        index: 0,
        codec: "h264".into(),
        width: SIZE.0,
        height: SIZE.1,
        frame_rate_num: 24,
        frame_rate_den: 1,
        start_time_s: 0.0,
        color_range: Some("pc".into()),
        ..Default::default()
    }
}

/// A scripted video: its frames in order, then the stills asked for.
pub struct Source {
    pub writings: Vec<Writing>,
    pub timeline: Vec<(f64, f64)>,
    pub next: u64,
    pub finished: bool,
    /// Whether a corner patch that is not writing changes on every frame.
    pub flicker: bool,
    pub stills: Vec<Vec<u64>>,
}

impl Source {
    pub fn new(writings: Vec<Writing>, count: u64) -> Self {
        Self {
            writings,
            timeline: timeline(count),
            next: 0,
            finished: false,
            flicker: false,
            stills: Vec::new(),
        }
    }
}

impl FrameSource for Source {
    fn frame_size(&self) -> (u32, u32) {
        SIZE
    }

    fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    fn next_frame(&mut self) -> TextResult<Option<YuvFrame>> {
        if self.next as usize >= self.timeline.len() {
            return Ok(None);
        }
        let frame = yuv_frame(&self.writings, self.next, self.flicker, &self.timeline);
        self.next += 1;
        Ok(Some(frame))
    }

    fn finish(&mut self) -> TextResult<()> {
        self.finished = true;
        Ok(())
    }

    fn stills(&mut self, indices: &[u64]) -> TextResult<Vec<RgbImage>> {
        assert!(self.finished, "stills follow the finished frame stream");
        assert!(indices.len() <= 8, "stills come eight at a time");
        self.stills.push(indices.to_vec());
        Ok(indices
            .iter()
            .map(|&index| still(&self.writings, index, self.flicker))
            .collect())
    }
}

/// Which waiting screening job the scripted sessions answer next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The oldest first.
    Oldest,
    /// The newest first.
    Newest,
    /// The second oldest when two wait, as a second session finishing first would.
    SecondOldest,
}

/// One answered job: its number, priority, and how many screening jobs still waited then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Answered {
    pub seq: u64,
    pub priority: Priority,
    pub screens_waiting: usize,
}

/// Scripted detector sessions: they find the scripted writing whose top-left pixel shows ink.
pub struct Pool {
    pub writings: Vec<Writing>,
    pub batch: usize,
    pub sessions: usize,
    pub answer: Answer,
    pub queue: Vec<ScreenJob>,
    pub answered: Vec<Answered>,
    pub screened: usize,
    /// Per confirmation call, the ink of the writing each frame shows, in job order.
    pub confirmations: Vec<Vec<Vec<u8>>>,
}

impl Pool {
    pub fn new(writings: Vec<Writing>, sessions: usize, answer: Answer) -> Self {
        Self {
            writings,
            batch: 4,
            sessions,
            answer,
            queue: Vec::new(),
            answered: Vec::new(),
            screened: 0,
            confirmations: Vec::new(),
        }
    }

    /// The regions found on `frame`, moved by `offset`.
    fn found(&self, frame: &PaddedFrame, offset: f64) -> Vec<(Quad, f64)> {
        self.shown(frame)
            .into_iter()
            .map(|writing| (region(writing, offset), 0.9))
            .collect()
    }

    /// The inks of the writing `frame` shows.
    fn inks(&self, frame: &PaddedFrame) -> Vec<u8> {
        self.shown(frame)
            .into_iter()
            .map(|writing| writing.ink)
            .collect()
    }

    /// The scripted writings `frame` shows: those whose top-left pixel shows their ink.
    fn shown(&self, frame: &PaddedFrame) -> Vec<&Writing> {
        assert_eq!((frame.width, frame.height), SIZE, "full-resolution frames");
        assert_eq!(frame.padded_height % 32, 0);
        let row = frame.width as usize * 3;
        assert!(
            frame.rgb[frame.height as usize * row..]
                .iter()
                .all(|&value| value == 0),
            "padding rows are black"
        );
        self.writings
            .iter()
            .filter(|writing| {
                let (left, top, _, _) = writing.rect;
                let at = top as usize * row + left as usize * 3;
                frame.rgb[at..at + 3] == [writing.ink; 3]
            })
            .collect()
    }
}

impl TextScreening for Pool {
    fn shape(&self) -> ScreenShape {
        ScreenShape {
            batch: self.batch,
            pool_mib: 0,
        }
    }

    fn sessions(&self) -> usize {
        self.sessions
    }

    fn submit(&mut self, job: ScreenJob) -> Result<(), OcrError> {
        assert!(job.frames.len() <= self.batch, "a job fits one batch");
        self.queue.push(job);
        Ok(())
    }

    fn recv(&mut self) -> Result<ScreenResult, OcrError> {
        let screens: Vec<usize> = (0..self.queue.len())
            .filter(|&at| self.queue[at].priority == Priority::Screen)
            .collect();
        let probe = self
            .queue
            .iter()
            .position(|job| job.priority == Priority::Probe);
        let at = match (probe, self.answer) {
            (Some(at), _) => at,
            (None, Answer::Oldest) => *screens.first().ok_or("no job waits")?,
            (None, Answer::Newest) => *screens.last().ok_or("no job waits")?,
            (None, Answer::SecondOldest) => {
                *screens.get(1).or(screens.first()).ok_or("no job waits")?
            }
        };
        let job = self.queue.remove(at);
        self.answered.push(Answered {
            seq: job.seq,
            priority: job.priority,
            screens_waiting: self
                .queue
                .iter()
                .filter(|job| job.priority == Priority::Screen)
                .count(),
        });
        self.screened += job.frames.len();
        Ok(ScreenResult {
            seq: job.seq,
            regions: job
                .frames
                .iter()
                .map(|frame| self.found(frame, 0.0))
                .collect(),
        })
    }

    fn confirm(&mut self, jobs: Vec<ConfirmJob>) -> Result<Vec<ConfirmResult>, OcrError> {
        assert!(self.queue.is_empty(), "screening ends before confirmation");
        self.confirmations
            .push(jobs.iter().map(|job| self.inks(&job.frame)).collect());
        Ok(jobs
            .iter()
            .map(|job| ConfirmResult {
                seq: job.seq,
                regions: self.found(&job.frame, EXACT_OFFSET),
            })
            .collect())
    }

    fn warmup_s(&self) -> f64 {
        1.5
    }

    fn engine_build_s(&self) -> f64 {
        0.0
    }

    fn notes(&self) -> BTreeMap<String, String> {
        BTreeMap::from([("engine".to_string(), "scripted".to_string())])
    }
}
