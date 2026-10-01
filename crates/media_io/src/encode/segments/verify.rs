//! The checks a joined localized video passes before it replaces the whole-video encode.
//!
//! **Role:** compare the joined file with its source: every frame present at its time, the video
//! starting at the same distance from the audio, a clean decode through every join, and the
//! copied pieces' packets unchanged; say which check failed.
//! **Position:** inside `encode::segments`; the localize stage verifies the joined `.part` file
//! and falls back to the whole-video encode on any reason.
//! **Signals and state:** bounded ffprobe and FFmpeg runs over the source and the joined file; the
//! packet hash lists are written in the caller's folder and removed.
//! **Invariants:** fail closed: a check that cannot run is a reason too. Timing tolerances are half
//! a frame; a decode counts as clean only when FFmpeg exits 0 and prints nothing at error level.

use std::path::Path;
use std::time::Duration;

use child_process::Run;

use super::FallbackReason;
use super::copied::copied_packets_match;
use super::idr::IdrProbe;
use super::plan::Piece;
use super::source::{StreamStarts, probe_h264_source};
use crate::encode::path;
use crate::video_frames::packets::keyframe_packets;
use crate::video_frames::timeline;
use crate::{MediaError, Programs};

/// How long before each join a decode check starts, at the latest.
pub const DECODE_LEAD_S: f64 = 2.0;
/// How long after each join a decode check runs.
pub const DECODE_LAG_S: f64 = 2.0;
/// How many keyframes before each join are tried as the decode check's IDR start.
const START_CANDIDATES: usize = 3;

/// What the checks compare.
pub struct VerifyRequest<'a> {
    /// The source video, only read.
    pub source: &'a Path,
    /// The source's timeline, from `video_frames::timeline`.
    pub source_timeline: &'a [(f64, f64)],
    /// When the source's video and audio start (`H264Source::starts`).
    pub source_starts: StreamStarts,
    /// The source's constant frame rate as a fraction.
    pub frame_rate: (u32, u32),
    /// The pieces the joined file was made of.
    pub pieces: &'a [Piece],
    /// The joined file.
    pub output: &'a Path,
    /// A folder for the packet hash lists.
    pub folder: &'a Path,
    /// The deadline of each FFmpeg run.
    pub timeout: Duration,
}

/// Every check, cheapest first: `Ok` when the joined file may be installed, else why not.
pub fn verify_join(programs: &Programs, request: &VerifyRequest) -> Result<(), FallbackReason> {
    let (num, den) = request.frame_rate;
    if num == 0 || den == 0 {
        return Err(FallbackReason("the frame rate is unknown".into()));
    }
    let frame_s = f64::from(den) / f64::from(num);
    let joined = probe_h264_source(programs, request.output).map_err(unreadable)?;
    let output_timeline =
        timeline(programs, request.output, 0.0, 1.0 / frame_s).map_err(unreadable)?;
    compare_timelines(request.source_timeline, &output_timeline, frame_s)?;
    compare_starts(request.source_starts, joined.starts, frame_s)?;
    let format = joined
        .packet_format
        .ok_or_else(|| FallbackReason("the joined video's packet format is unknown".into()))?;
    let keyframes = keyframe_packets(programs, request.output).map_err(unreadable)?;
    let mut probe = IdrProbe::new(programs, request.output, format, keyframes.clone(), frame_s);
    let keyframe_indices: Vec<u64> = keyframes.iter().map(|keyframe| keyframe.index).collect();
    let lead = (DECODE_LEAD_S / frame_s).ceil() as u64;
    let lag = (DECODE_LAG_S / frame_s).ceil() as u64;
    let candidates = start_candidates(request.pieces, &keyframe_indices, lead);
    probe.confirm(&candidates).map_err(unreadable)?;
    let mut starts = probe.confirmed();
    starts.extend(request.pieces.iter().map(|piece| piece.first()));
    starts.sort_unstable();
    let windows = decode_windows(
        request.pieces,
        output_timeline.len() as u64,
        lead,
        lag,
        &starts,
    );
    for (first, end) in windows {
        decode_cleanly(programs, request, &output_timeline, first, end, frame_s)?;
    }
    copied_packets_match(programs, request)
}

fn unreadable(error: MediaError) -> FallbackReason {
    FallbackReason(format!("the joined video could not be checked: {error}"))
}

/// Every frame of the joined file presents where the source's does, relative to the first, to
/// within half a frame, and the last ends there too.
pub(super) fn compare_timelines(
    source: &[(f64, f64)],
    joined: &[(f64, f64)],
    frame_s: f64,
) -> Result<(), FallbackReason> {
    if source.len() != joined.len() {
        return Err(FallbackReason(format!(
            "the joined video has {} frames, the source {}",
            joined.len(),
            source.len()
        )));
    }
    let (Some(&(source_first, _)), Some(&(joined_first, _))) = (source.first(), joined.first())
    else {
        return Ok(());
    };
    let tolerance = frame_s / 2.0;
    for (index, (&(source_start, source_end), &(joined_start, joined_end))) in
        source.iter().zip(joined).enumerate()
    {
        let start_drift = (joined_start - joined_first) - (source_start - source_first);
        let end_drift = (joined_end - joined_first) - (source_end - source_first);
        let last = index + 1 == source.len();
        if start_drift.abs() > tolerance || (last && end_drift.abs() > tolerance) {
            return Err(FallbackReason(format!(
                "frame {index} of the joined video is {start_drift:+.3} s off the source's time"
            )));
        }
    }
    Ok(())
}

/// The joined video starts the same distance after its audio as the source's, within half a
/// frame; a source with audio needs audio in the joined file.
pub(super) fn compare_starts(
    source: StreamStarts,
    joined: StreamStarts,
    frame_s: f64,
) -> Result<(), FallbackReason> {
    match (source.video_after_audio_s(), joined.video_after_audio_s()) {
        (None, _) if source.audio_s.is_none() => Ok(()),
        (Some(source_offset), Some(joined_offset))
            if (joined_offset - source_offset).abs() <= frame_s / 2.0 =>
        {
            Ok(())
        }
        (source_offset, joined_offset) => Err(FallbackReason(format!(
            "the joined video starts {} s after its audio, the source's {} s",
            seconds(joined_offset),
            seconds(source_offset)
        ))),
    }
}

fn seconds(value: Option<f64>) -> String {
    value.map_or_else(|| "unknown".into(), |value| format!("{value:.3}"))
}

/// The keyframes worth asking about as decode starts: for each join, the last few keyframes at
/// least `lead` frames before it, ascending and without repeats.
pub(super) fn start_candidates(pieces: &[Piece], keyframes: &[u64], lead: u64) -> Vec<u64> {
    let mut candidates: Vec<u64> = pieces
        .iter()
        .skip(1)
        .flat_map(|piece| {
            let latest = piece.first().saturating_sub(lead);
            let end = keyframes.partition_point(|&key| key <= latest);
            keyframes[end.saturating_sub(START_CANDIDATES)..end].to_vec()
        })
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

/// The frame windows `[first, end)` to decode around every join: from the last decode start in
/// `starts` (ascending, holding 0) at least `lead` frames before the join to `lag` frames after
/// it, overlapping windows joined.
pub(super) fn decode_windows(
    pieces: &[Piece],
    frame_count: u64,
    lead: u64,
    lag: u64,
    starts: &[u64],
) -> Vec<(u64, u64)> {
    let mut windows: Vec<(u64, u64)> = Vec::new();
    for join in pieces.iter().skip(1).map(|piece| piece.first()) {
        let latest = join.saturating_sub(lead);
        let first = starts[..starts.partition_point(|&start| start <= latest)]
            .last()
            .copied()
            .unwrap_or(0);
        let end = join.saturating_add(lag).min(frame_count);
        match windows.last_mut() {
            Some(last) if first <= last.1 => last.1 = last.1.max(end),
            _ => windows.push((first, end)),
        }
    }
    windows
}

/// Decode frames `[first, end)` of the joined file from the keyframe at `first`; any error FFmpeg
/// prints is a reason.
fn decode_cleanly(
    programs: &Programs,
    request: &VerifyRequest,
    timeline: &[(f64, f64)],
    first: u64,
    end: u64,
    frame_s: f64,
) -> Result<(), FallbackReason> {
    let start_s = timeline
        .get(first as usize)
        .map_or(0.0, |&(start, _)| start);
    let end_s = timeline.get(end as usize).map_or_else(
        || timeline.last().map_or(0.0, |&(_, end)| end),
        |&(start, _)| start,
    );
    let seek_s = start_s + frame_s / 4.0;
    let output = Run::new(&programs.ffmpeg)
        .args(decode_args(
            request.output,
            seek_s,
            (end_s - seek_s).max(frame_s),
        ))
        .timeout(request.timeout)
        .output()
        .map_err(|error| unreadable(error.into()))?;
    let complaint = output.stderr.trim();
    if output.code != 0 || !complaint.is_empty() {
        return Err(FallbackReason(format!(
            "the joined video does not decode cleanly from {start_s:.3} s: {}",
            complaint.lines().next().unwrap_or("FFmpeg failed")
        )));
    }
    Ok(())
}

/// FFmpeg's arguments to decode `duration_s` of the first video stream from the keyframe at or
/// before `seek_s`, to nowhere, printing only errors.
pub(super) fn decode_args(video: &Path, seek_s: f64, duration_s: f64) -> Vec<String> {
    vec![
        "-nostdin".into(),
        "-hide_banner".into(),
        "-v".into(),
        "error".into(),
        "-ss".into(),
        format!("{seek_s:.6}"),
        "-i".into(),
        path(video),
        "-t".into(),
        format!("{duration_s:.6}"),
        "-map".into(),
        "0:v:0".into(),
        "-f".into(),
        "null".into(),
        "-".into(),
    ]
}

#[cfg(test)]
#[path = "tests/verify.rs"]
mod tests;
