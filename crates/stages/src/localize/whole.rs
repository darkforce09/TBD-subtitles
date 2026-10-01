//! The whole-video encode: every frame decoded, blended and re-encoded.
//!
//! **Role:** write the localized video from every frame of the source with the best available
//! encoder (NVENC HEVC when a test encode runs, else libx264), keeping the source's audio,
//! chapters and metadata; the route when the segment encode cannot be used.
//! **Position:** inside `localize`; `render` runs it after the segment encode gave a reason.
//! **Signals and state:** one decode thread (`frames::Decoder`) and one encoder thread
//! (`threads::run_frames`) for the run.
//! **Invariants:** the encoder receives exactly one frame per timeline entry, in order, at the
//! source's constant frame rate and first-frame offset; the record carries the reason the whole
//! video was encoded.

use std::path::Path;
use std::time::Instant;

use job_model::onscreen::SegmentSummary;
use media_io::encode::segments::FallbackReason;
use media_io::encode::{EncodeSpec, EncoderProcess, VideoColour, available_encoder};
use media_io::video_frames::PixelFormat;

use super::frames::Decoder;
use super::threads::{FrameRun, run_frames};
use super::{
    Blender, ENCODE_DEADLINE, LocalizeResult, PROGRESS_FRAMES, Progress, RenderPhases, Rendered,
    Source,
};

/// Encode every frame of `source` with `blender`'s patches, adding to `phases`; `reason` says why
/// the segments were not used.
pub fn encode(
    source: &Source,
    blender: &mut Blender,
    progress: Progress,
    mut phases: RenderPhases,
    reason: FallbackReason,
) -> LocalizeResult<Rendered> {
    let request = source.request;
    let total = source.timeline.len() as u64;
    if total == 0 {
        return Err("the video has no frames".into());
    }
    let encoder = available_encoder(request.programs)?;
    let stream = request.stream;
    let spec = EncodeSpec {
        width: source.size.0,
        height: source.size.1,
        frame_rate: (stream.frame_rate_num, stream.frame_rate_den),
        pixel_format: source.format,
        video_offset_s: source.timeline.first().map_or(0.0, |&(start, _)| start),
        colour: VideoColour::of(stream),
        source_bit_rate: stream
            .bit_rate
            .or_else(|| whole_file_rate(request.video, &source.timeline)),
        source: request.video.to_path_buf(),
        output: request.output.to_path_buf(),
        encoder,
    };
    let mut decoder = match source.format {
        PixelFormat::Yuv420p => Decoder::yuv(
            request.programs,
            request.video,
            source.size,
            source.fps,
            source.timeline.clone(),
            0,
            None,
        )?,
        format => Decoder::native(
            request.programs,
            request.video,
            source.size,
            stream.start_time_s,
            source.fps,
            format,
            vec![(0, total - 1)],
        )?,
    };
    let cancel = request.cancel.clone();
    let mut done = 0;
    let written = run_frames(
        FrameRun {
            first: 0,
            count: total,
        },
        &mut || decoder.recv(),
        &mut |index, frame| blender.apply(index, frame),
        || EncoderProcess::start(request.programs, &spec, ENCODE_DEADLINE, cancel),
        request.cancel.as_deref(),
        &mut phases,
        &mut || {
            done += 1;
            if done % PROGRESS_FRAMES == 0 {
                progress(done, total as usize);
            }
        },
    )?;
    let flushing = Instant::now();
    decoder.finish()?;
    phases.flush += flushing.elapsed();
    progress(total as usize, total as usize);
    Ok(Rendered {
        frames: written,
        encoder: encoder.name(),
        phases,
        segments: whole_summary(total, reason),
    })
}

/// The record of a whole-video encode: one segment of every frame, none copied, and the reason.
pub fn whole_summary(frames: u64, reason: FallbackReason) -> SegmentSummary {
    SegmentSummary {
        segments_reencoded: 1,
        frames_reencoded: frames,
        frames_copied: 0,
        fallback_reason: Some(reason.0),
    }
}

/// The whole file's average bit rate, for probes that predate the stream's own: its size over the
/// timeline's length.
fn whole_file_rate(video: &Path, timeline: &[(f64, f64)]) -> Option<u64> {
    let bytes = std::fs::metadata(video).ok()?.len();
    let seconds = timeline.last()?.1 - timeline.first()?.0;
    (seconds > 0.0).then(|| (bytes as f64 * 8.0 / seconds).round() as u64)
}
