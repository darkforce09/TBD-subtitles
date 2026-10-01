//! The segment encode: only the segments with replaced writing are decoded, blended and
//! re-encoded as H.264 matching the source; every other frame's packets are copied.
//!
//! **Role:** judge the source, plan pieces around the frames any patch covers, cut the copied
//! pieces, render each re-encoded piece through the decode → blend → encode threads, join the
//! pieces with the source's audio, and check the joined file; or say why the whole video must be
//! encoded instead.
//! **Position:** inside `localize`; `render` tries it first and falls back to `whole` on a
//! `FallbackReason`. The media work is `media_io::encode::segments`.
//! **Signals and state:** the request's `pieces_dir`, made afresh for the pieces and removed
//! when the attempt ends; one decode thread per re-encoded piece for 8-bit sources, one
//! for the whole attempt for 10-bit ones; one encoder thread per re-encoded piece.
//! **Invariants:** the source is only read; the output is left in place only after
//! `verify_join` passed; any doubt (an ineligible source, a failed probe, plan, copy, encode, join
//! or check, or a plan with nothing to copy) is a fallback with its reason, except a cancel,
//! which stops the render.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use job_model::onscreen::LocalizedEncoder;
use media_io::MediaError;
use media_io::encode::EncoderProcess;
use media_io::encode::segments::{
    FallbackReason, FrameSpan, H264Source, IdrProbe, JoinRequest, Piece, SegmentSpec,
    VerifyRequest, copy_pieces, join_pieces, keyframe_packets, piece_file, piece_summary,
    plan_pieces, probe_h264_source, segment_eligibility, segment_encoder, verify_join,
};
use media_io::video_frames::PixelFormat;

use super::frames::Decoder;
use super::threads::{FrameRun, run_frames};
use super::{
    Blender, ENCODE_DEADLINE, LocalizeError, PROGRESS_FRAMES, Progress, RenderPhases, Rendered,
    Source,
};

/// The longest the checks of a joined video may run.
const VERIFY_DEADLINE: Duration = Duration::from_secs(3600);

/// Why a segment encode wrote no localized video.
#[derive(Debug)]
pub enum Attempt {
    /// The whole video is encoded instead, for this reason.
    Fallback(FallbackReason),
    /// The render stops: cancelled, or a failure no other route avoids.
    Failed(LocalizeError),
}

/// Try the segment encode of `source` with `blender`'s patches, adding the time it spends to
/// `phases` whether it succeeds or not.
pub fn encode(
    source: &Source,
    blender: &mut Blender,
    progress: Progress,
    phases: &mut RenderPhases,
) -> Result<Rendered, Attempt> {
    let planning = Instant::now();
    let planned = plan(source, &blender.changed_spans());
    phases.plan += planning.elapsed();
    let fall_back = |reason: FallbackReason| {
        let _ = std::fs::remove_file(source.request.output);
        cancelled(source).unwrap_or(Attempt::Fallback(reason))
    };
    let (h264, pieces) = planned.map_err(fall_back)?;
    let folder = fresh_folder(source.request.pieces_dir).map_err(fall_back)?;
    let built = build(source, blender, &h264, &pieces, &folder, phases, progress);
    let _ = std::fs::remove_dir_all(&folder);
    let encoder = built.map_err(fall_back)?;
    Ok(Rendered {
        frames: source.timeline.len() as u64,
        encoder,
        phases: *phases,
        segments: piece_summary(&pieces),
    })
}

/// The source's H.264 stream and the pieces its `changed` frames make, or why there are none.
fn plan(
    source: &Source,
    changed: &[(u64, u64)],
) -> Result<(H264Source, Vec<Piece>), FallbackReason> {
    let (programs, video) = (source.request.programs, source.request.video);
    let h264 = probe_h264_source(programs, video).map_err(failed("probe the video stream"))?;
    let keys = keyframe_packets(programs, video).map_err(failed("list the keyframes"))?;
    let keyframes: Vec<u64> = keys.iter().map(|key| key.index).collect();
    segment_eligibility(&h264, &source.timeline, &keyframes)?;
    let packets = h264
        .packet_format
        .ok_or_else(|| FallbackReason("the H.264 packet format is unknown".into()))?;
    let mut probe = IdrProbe::new(programs, video, packets, keys, h264.frame_s());
    let pieces = plan_pieces(
        &frame_spans(changed),
        &keyframes,
        source.timeline.len() as u64,
        &mut |batch| probe.confirm(batch),
    )
    .map_err(|error| FallbackReason(error.to_string()))?;
    worth_joining(&pieces)?;
    Ok((h264, pieces))
}

/// The changed runs `(first, last)` as the planner's spans.
pub fn frame_spans(changed: &[(u64, u64)]) -> Vec<FrameSpan> {
    changed
        .iter()
        .map(|&(first, last)| FrameSpan { first, last })
        .collect()
}

/// Whether a plan copies anything: a plan that re-encodes every frame gains nothing from pieces,
/// and the whole-video encode does it with fewer joins.
pub fn worth_joining(pieces: &[Piece]) -> Result<(), FallbackReason> {
    if pieces.iter().any(|piece| piece.is_copy()) {
        Ok(())
    } else {
        Err(FallbackReason(
            "every frame lies in a re-encoded segment".into(),
        ))
    }
}

/// Copy, render, join and check the pieces in `folder`; the encoder that re-encoded them.
fn build(
    source: &Source,
    blender: &mut Blender,
    h264: &H264Source,
    pieces: &[Piece],
    folder: &Path,
    phases: &mut RenderPhases,
    progress: Progress,
) -> Result<&'static str, FallbackReason> {
    let request = source.request;
    let (programs, video) = (request.programs, request.video);
    let copying = Instant::now();
    copy_pieces(
        programs,
        video,
        pieces,
        folder,
        ENCODE_DEADLINE,
        request.cancel.clone(),
    )
    .map_err(failed("copy the unchanged pieces"))?;
    phases.copy = copying.elapsed();
    let encoder = if pieces.iter().all(|piece| piece.is_copy()) {
        None
    } else {
        Some(segment_encoder(programs, request.encoder, h264))
    };
    let used = match encoder {
        Some(encoder) => encode_pieces(
            source, blender, h264, pieces, folder, encoder, phases, progress,
        )?,
        None => "copy",
    };
    let joining = Instant::now();
    let output = request.output;
    let join = JoinRequest {
        folder: folder.to_path_buf(),
        source: video.to_path_buf(),
        video_offset_s: source.timeline.first().map_or(0.0, |&(start, _)| start),
        frame_rate: h264.frame_rate,
        output: output.to_path_buf(),
    };
    join_pieces(
        programs,
        pieces,
        &join,
        ENCODE_DEADLINE,
        request.cancel.clone(),
    )
    .map_err(failed("join the pieces"))?;
    phases.join = joining.elapsed();
    let verifying = Instant::now();
    verify_join(
        programs,
        &VerifyRequest {
            source: video,
            source_timeline: &source.timeline,
            source_starts: h264.starts,
            frame_rate: h264.frame_rate,
            pieces,
            output,
            folder,
            timeout: VERIFY_DEADLINE,
        },
    )?;
    phases.verify = verifying.elapsed();
    Ok(used)
}

/// Decode, blend and encode every re-encoded piece into its file in `folder` with `encoder`; the
/// FFmpeg name of the encoder that ran.
#[allow(clippy::too_many_arguments)]
fn encode_pieces(
    source: &Source,
    blender: &mut Blender,
    h264: &H264Source,
    pieces: &[Piece],
    folder: &Path,
    encoder: LocalizedEncoder,
    phases: &mut RenderPhases,
    progress: Progress,
) -> Result<&'static str, FallbackReason> {
    let request = source.request;
    let (programs, video) = (request.programs, request.video);
    let encoded: Vec<(usize, Piece)> = pieces
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, piece)| !piece.is_copy())
        .collect();
    let total: u64 = encoded.iter().map(|(_, piece)| piece.frames()).sum();
    // A 10-bit source decodes through one stream from the start, handing out only these frames.
    let mut shared = match source.format {
        PixelFormat::Yuv420p => None,
        format => Some(
            Decoder::native(
                programs,
                video,
                source.size,
                request.stream.start_time_s,
                source.fps,
                format,
                encoded
                    .iter()
                    .map(|(_, piece)| (piece.first(), piece.last()))
                    .collect(),
            )
            .map_err(failed("start the decoder"))?,
        ),
    };
    let mut used = encoder_name(encoder);
    let mut done = 0;
    for (number, piece) in encoded {
        let spec = SegmentSpec::for_source(
            h264,
            source.format,
            encoder,
            piece_file(folder, number, piece),
        )?;
        used = encoder_name(spec.encoder);
        let mut own = match shared {
            Some(_) => None,
            None => Some(
                Decoder::yuv(
                    programs,
                    video,
                    source.size,
                    source.fps,
                    source.timeline.clone(),
                    piece.first(),
                    Some(piece.frames()),
                )
                .map_err(failed("start the decoder"))?,
            ),
        };
        let Some(decoder) = own.as_mut().or(shared.as_mut()) else {
            return Err(FallbackReason("no decoder for the segment".into()));
        };
        let cancel = request.cancel.clone();
        run_frames(
            FrameRun {
                first: piece.first(),
                count: piece.frames(),
            },
            &mut || decoder.recv(),
            &mut |index, frame| blender.apply(index, frame),
            || EncoderProcess::start_segment(programs, &spec, ENCODE_DEADLINE, cancel),
            request.cancel.as_deref(),
            phases,
            &mut || {
                done += 1;
                if done % PROGRESS_FRAMES == 0 {
                    progress(done, total as usize);
                }
            },
        )
        .map_err(|error| FallbackReason(format!("the segment encode failed: {error}")))?;
        let flushing = Instant::now();
        if let Some(own) = own {
            own.finish().map_err(failed("decode the segment"))?;
        }
        phases.flush += flushing.elapsed();
    }
    let flushing = Instant::now();
    if let Some(shared) = shared {
        shared.finish().map_err(failed("decode the segments"))?;
    }
    phases.flush += flushing.elapsed();
    progress(total as usize, total as usize);
    Ok(used)
}

/// FFmpeg's name for a segment encoder.
pub fn encoder_name(encoder: LocalizedEncoder) -> &'static str {
    match encoder {
        LocalizedEncoder::X264 => "libx264",
        LocalizedEncoder::Nvenc => "h264_nvenc",
    }
}

/// A media failure doing `what` as a fallback reason.
fn failed(what: &'static str) -> impl Fn(MediaError) -> FallbackReason {
    move |error| FallbackReason(format!("could not {what}: {error}"))
}

/// The stop for a cancelled render, which no fallback outlives.
fn cancelled(source: &Source) -> Option<Attempt> {
    let flag = source.request.cancel.as_ref()?;
    flag.load(Ordering::SeqCst)
        .then(|| Attempt::Failed("the localized video was cancelled".into()))
}

/// The work folder for the pieces, emptied of any earlier attempt's files.
fn fresh_folder(folder: &Path) -> Result<PathBuf, FallbackReason> {
    if folder.exists() {
        std::fs::remove_dir_all(folder).map_err(|error| {
            FallbackReason(format!("could not empty {}: {error}", folder.display()))
        })?;
    }
    std::fs::create_dir_all(folder).map_err(|error| {
        FallbackReason(format!("could not create {}: {error}", folder.display()))
    })?;
    Ok(folder.to_path_buf())
}

#[cfg(test)]
#[path = "tests/segments.rs"]
mod tests;
