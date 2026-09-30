//! The localized video: every composed patch blended over the source frames and re-encoded.
//!
//! **Role:** decode every frame at its native size, blend the patches of baked occurrences over
//! the frames they cover, and stream the result into an encoder that keeps the source's audio,
//! chapters and metadata.
//! **Position:** the stage of the `localized_video` step, after composition; called by
//! `pipeline::tasks::localized`, above `media_io`'s native frame stream and encoder.
//! **Signals and state:** one FFmpeg decoder and one FFmpeg encoder, a patch schedule and a patch
//! cache bounded to `CACHE_BYTES`; one frame in memory at a time.
//! **Invariants:** the source is only read; the encoder receives exactly one frame per timeline
//! entry, in order, at the source's constant frame rate and first-frame offset; a variable frame
//! rate is refused before encoding; frames without an active patch pass through unchanged.

pub mod blend;
pub mod colour;
pub mod patches;
pub mod still;

use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use job_model::onscreen::ReplacementDocument;
use job_model::outputs::VideoStream;
use media_io::encode::{
    EncodeSpec, Encoder, EncoderProcess, VideoColour, available_encoder, is_constant_frame_rate,
};
use media_io::video_frames::{Decode, FrameStream, PixelFormat};
use media_io::{MediaError, Programs};

use blend::blend;
use colour::Conversion;
use patches::{PatchCache, Schedule, load};

/// The most converted patch samples held at once.
pub const CACHE_BYTES: usize = 512 * 1024 * 1024;
/// Frames between two progress reports.
pub const PROGRESS_FRAMES: usize = 240;
/// The longest an encode may run.
const ENCODE_DEADLINE: Duration = Duration::from_secs(24 * 3600);

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
    /// The job directory.
    pub root: &'a Path,
    /// The Matroska file to write.
    pub output: &'a Path,
    /// Stops the encode once set.
    pub cancel: Option<Arc<AtomicBool>>,
}

/// What a render wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rendered {
    pub frames: u64,
    pub encoder: Encoder,
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
/// encoded with the best available encoder. Returns the frames written and the encoder.
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
    let mut schedule = Schedule::new(request.document);
    let document = request.document;
    if !schedule.is_empty() && (document.width, document.height) != size {
        return Err(format!(
            "the replacements are for a {}x{} video, not {}x{}",
            document.width, document.height, size.0, size.1
        )
        .into());
    }
    let encoder = available_encoder(request.programs)?;
    let mut frames = FrameStream::open_native(
        request.programs,
        request.video,
        size,
        stream.start_time_s,
        fps,
        format,
        Decode::Exact,
    )?;
    let total = frames.timeline().len();
    if !is_constant_frame_rate(frames.timeline(), fps) {
        return Err(
            "The video has a variable frame rate, which the localized video cannot keep in sync"
                .into(),
        );
    }
    if !schedule.is_empty() && document.frame_count != total as u64 {
        return Err(format!(
            "the replacements are for {} frames, but the video has {total}",
            document.frame_count
        )
        .into());
    }
    let spec = EncodeSpec {
        width: size.0,
        height: size.1,
        frame_rate: (num, den),
        pixel_format: format,
        video_offset_s: frames.timeline().first().map_or(0.0, |&(start, _)| start),
        colour: VideoColour::of(stream),
        source_bit_rate: stream
            .bit_rate
            .or_else(|| whole_file_rate(request.video, frames.timeline())),
        source: request.video.to_path_buf(),
        output: request.output.to_path_buf(),
        encoder,
    };
    let mut encode = EncoderProcess::start(
        request.programs,
        &spec,
        ENCODE_DEADLINE,
        request.cancel.clone(),
    )?;
    let conversion = Conversion::of(stream, format);
    let mut cache = PatchCache::new(CACHE_BYTES);
    let mut done = 0;
    while let Some(mut frame) = frames.next_frame()? {
        if request
            .cancel
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            return Err("the localized video was cancelled".into());
        }
        for order in schedule.advance(frame.index)? {
            cache.remove(order);
        }
        for entry in schedule.active() {
            let patch = cache.get(entry, |entry| load(request.root, entry, conversion))?;
            blend(&mut frame.rgb, format, size, patch)?;
        }
        if let Err(error) = encode.write_frame(&frame.rgb) {
            // The encoder stopped reading; its exit says why.
            encode.finish()?;
            return Err(error.into());
        }
        done += 1;
        if done % PROGRESS_FRAMES == 0 {
            progress(done, total);
        }
    }
    frames.finish()?;
    let written = encode.finish()?;
    if written != total as u64 {
        return Err(format!("the encoder took {written} frames of {total}").into());
    }
    progress(total, total);
    Ok(Rendered {
        frames: written,
        encoder,
    })
}

/// The whole file's average bit rate, for probes that predate the stream's own: its size over the
/// timeline's length.
fn whole_file_rate(video: &Path, timeline: &[(f64, f64)]) -> Option<u64> {
    let bytes = std::fs::metadata(video).ok()?.len();
    let seconds = timeline.last()?.1 - timeline.first()?.0;
    (seconds > 0.0).then(|| (bytes as f64 * 8.0 / seconds).round() as u64)
}

#[cfg(test)]
#[path = "tests/render.rs"]
mod tests;
