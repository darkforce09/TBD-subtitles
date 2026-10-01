//! The localized video: every composed patch blended over the source frames, with only the
//! segments that change re-encoded when the source allows it.
//!
//! **Role:** blend the patches of baked occurrences over the frames they cover, each frame the
//! patch lettered at its own shift, and write a video that keeps the source's audio, chapters
//! and metadata: from pieces, the changed segments re-encoded as H.264 matching the source and the
//! rest copied, or, on any doubt, every frame re-encoded.
//! **Position:** the stage of the `localized_video` step, after composition; called by
//! `pipeline::tasks::localized`, above `media_io`'s frame streams, frame queue and encoders.
//! **Signals and state:** a decode thread, the step thread blending and an encoder thread per
//! encode (`threads`); a patch schedule and a patch cache bounded to `CACHE_BYTES`; a work folder
//! for the pieces, removed afterwards; the time each thread waits and works (`RenderPhases`).
//! **Invariants:** the source is only read; every encode receives exactly one frame per timeline
//! entry it covers, in order, at the source's constant frame rate; a variable frame rate is
//! refused before encoding; frames without an active patch pass through unchanged; a video built
//! from pieces is written only after every segment check passed, else the whole video is encoded
//! and the reason recorded.

pub mod blend;
pub mod colour;
pub mod frames;
pub mod motion;
pub mod patches;
pub mod segments;
pub mod still;
pub mod threads;
pub mod whole;

use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use job_model::onscreen::{LocalizedEncoder, ReplacementDocument, SegmentSummary};
use job_model::outputs::VideoStream;
use media_io::encode::is_constant_frame_rate;
use media_io::encode::segments::FallbackReason;
use media_io::video_frames::{PixelFormat, timeline};
use media_io::{MediaError, Programs};

use blend::blend;
use colour::Conversion;
use motion::Motion;
use patches::{PatchCache, Schedule, load};
use segments::Attempt;

/// The most converted patch samples held at once.
pub const CACHE_BYTES: usize = 512 * 1024 * 1024;
/// Frames between two progress reports.
pub const PROGRESS_FRAMES: usize = 240;
/// The longest an encode, a copy or a join may run.
pub const ENCODE_DEADLINE: Duration = Duration::from_secs(24 * 3600);

/// Why the localized video could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizeError(pub String);

impl fmt::Display for LocalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LocalizeError {}

impl From<String> for LocalizeError {
    fn from(message: String) -> Self {
        LocalizeError(message)
    }
}

impl From<&str> for LocalizeError {
    fn from(message: &str) -> Self {
        LocalizeError(message.to_string())
    }
}

impl From<MediaError> for LocalizeError {
    fn from(error: MediaError) -> Self {
        LocalizeError(error.to_string())
    }
}

pub type LocalizeResult<T> = Result<T, LocalizeError>;

/// Where a render reports `(frames done, frames in all)`.
pub type Progress<'a> = &'a (dyn Fn(usize, usize) + Sync);

/// Everything one localized encode reads and writes.
pub struct RenderRequest<'a> {
    pub programs: &'a Programs,
    /// The source video, only read.
    pub video: &'a Path,
    /// The source's probed video stream.
    pub stream: &'a VideoStream,
    /// The composed replacements; patch paths are relative to `root`.
    pub document: &'a ReplacementDocument,
    /// The writing's shift in each frame, from the `frames` rows: which patch covers it.
    pub motion: &'a Motion,
    /// The job directory.
    pub root: &'a Path,
    /// The Matroska file to write.
    pub output: &'a Path,
    /// The segment encoder the settings ask for; x264 runs when NVENC cannot.
    pub encoder: LocalizedEncoder,
    /// The work folder the segment encode creates afresh for its pieces and removes afterwards;
    /// anything already there is deleted.
    pub pieces_dir: &'a Path,
    /// Stops the encode once set.
    pub cancel: Option<Arc<AtomicBool>>,
}

/// What a render wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub frames: u64,
    /// FFmpeg's name for the encoder that ran: `hevc_nvenc` or `libx264` for the whole video,
    /// `libx264` or `h264_nvenc` for segments, `copy` when no frame changed.
    pub encoder: &'static str,
    /// Where the render spent its time.
    pub phases: RenderPhases,
    /// How much was re-encoded and copied, and why the whole video was, when it was.
    pub segments: SegmentSummary,
}

/// Where a render spent its time. The frame phases add up over every encode the render ran.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderPhases {
    /// The step thread waiting on the decode thread for the next frame.
    pub decode_wait: Duration,
    /// The step thread advancing the patch schedule, loading patches and blending them in.
    pub blend: Duration,
    /// The step thread waiting for room in the encoder thread's queue.
    pub encode_wait: Duration,
    /// After each encode's last frame: draining the encoder thread and ending the decoder.
    pub flush: Duration,
    /// Probing the source's H.264 stream, its keyframes and IDR pictures, and planning pieces.
    pub plan: Duration,
    /// Copying the unchanged pieces out of the source.
    pub copy: Duration,
    /// Joining the pieces with the source's audio.
    pub join: Duration,
    /// Checking the joined video before it is kept.
    pub verify: Duration,
}

/// The source as every encode of one render reads it.
pub struct Source<'a> {
    pub request: &'a RenderRequest<'a>,
    pub size: (u32, u32),
    pub fps: f64,
    pub format: PixelFormat,
    /// Origin-relative (start, end) of every frame, by presentation index.
    pub timeline: Arc<[(f64, f64)]>,
}

/// The patches blended into each frame, loaded and converted as they come into use.
pub struct Blender<'a> {
    schedule: Schedule,
    cache: PatchCache,
    root: &'a Path,
    conversion: Conversion,
    format: PixelFormat,
    size: (u32, u32),
}

impl<'a> Blender<'a> {
    /// A blender over `schedule` for `format` frames of `size` from `stream`.
    pub fn new(
        schedule: Schedule,
        root: &'a Path,
        stream: &VideoStream,
        format: PixelFormat,
        size: (u32, u32),
    ) -> Blender<'a> {
        Blender {
            schedule,
            cache: PatchCache::new(CACHE_BYTES),
            root,
            conversion: Conversion::of(stream, format),
            format,
            size,
        }
    }

    /// The runs of frames any patch covers.
    pub fn changed_spans(&self) -> Vec<(u64, u64)> {
        self.schedule.changed_spans()
    }

    /// Blend every patch active at frame `index`, later than every frame before, into `frame`.
    pub fn apply(&mut self, index: u64, frame: &mut [u8]) -> LocalizeResult<()> {
        for file in self.schedule.advance(index)? {
            self.cache.remove(file);
        }
        let (root, conversion) = (self.root, self.conversion);
        for entry in self.schedule.active() {
            let patch = self
                .cache
                .get(entry, |entry| load(root, entry, conversion))?;
            blend(frame, self.format, self.size, patch)?;
        }
        Ok(())
    }
}

/// The raw format frames are decoded, blended and encoded in: 10-bit for a 10-bit 4:2:0 source,
/// else 8-bit 4:2:0, which FFmpeg converts any other source format into.
pub fn frame_format(stream: &VideoStream) -> PixelFormat {
    match stream.pix_fmt.as_deref() {
        Some("yuv420p10le" | "yuv420p10be" | "p010le" | "p010be") => PixelFormat::Yuv420p10le,
        _ => PixelFormat::Yuv420p,
    }
}

/// Write the localized video: every frame of the source with its active patches blended in,
/// from re-encoded segments and copied pieces when the source allows it, else encoded whole
/// with the best available encoder.
pub fn render(request: &RenderRequest, progress: Progress) -> LocalizeResult<Rendered> {
    let stream = request.stream;
    let (num, den) = (stream.frame_rate_num, stream.frame_rate_den);
    if num == 0 || den == 0 {
        return Err(
            "The video reports no frame rate, so the localized video cannot be timed".into(),
        );
    }
    let fps = f64::from(num) / f64::from(den);
    let size = (stream.width, stream.height);
    let format = frame_format(stream);
    let schedule = Schedule::new(request.document, request.motion)?;
    let document = request.document;
    if !schedule.is_empty() && (document.width, document.height) != size {
        return Err(format!(
            "the replacements are for a {}x{} video, not {}x{}",
            document.width, document.height, size.0, size.1
        )
        .into());
    }
    format.frame_bytes(size)?;
    let frames = timeline(request.programs, request.video, stream.start_time_s, fps)?;
    if !is_constant_frame_rate(&frames, fps) {
        return Err(
            "The video has a variable frame rate, which the localized video cannot keep in sync"
                .into(),
        );
    }
    let total = frames.len();
    if !schedule.is_empty() && document.frame_count != total as u64 {
        return Err(format!(
            "the replacements are for {} frames, but the video has {total}",
            document.frame_count
        )
        .into());
    }
    let source = Source {
        request,
        size,
        fps,
        format,
        timeline: frames.into(),
    };
    let mut blender = Blender::new(schedule, request.root, stream, format, size);
    let mut phases = RenderPhases::default();
    let attempt = segments::encode(&source, &mut blender, progress, &mut phases);
    settle(attempt, phases, |reason, phases| {
        let schedule = Schedule::new(request.document, request.motion)?;
        let mut blender = Blender::new(schedule, request.root, stream, format, size);
        whole::encode(&source, &mut blender, progress, phases, reason)
    })
}

/// The render's result from the segment `attempt`: its video, its stop, or the whole-video
/// encode `whole` runs with the attempt's reason and the `phases` it already spent.
pub fn settle(
    attempt: Result<Rendered, Attempt>,
    phases: RenderPhases,
    whole: impl FnOnce(FallbackReason, RenderPhases) -> LocalizeResult<Rendered>,
) -> LocalizeResult<Rendered> {
    match attempt {
        Ok(rendered) => Ok(rendered),
        Err(Attempt::Failed(error)) => Err(error),
        Err(Attempt::Fallback(reason)) => whole(reason, phases),
    }
}

#[cfg(test)]
#[path = "tests/render.rs"]
mod tests;
