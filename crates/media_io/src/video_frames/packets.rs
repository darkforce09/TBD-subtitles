//! The presentation timeline of a video's first video stream, read from its packet table, and
//! the frames its keyframe packets present.
//!
//! **Role:** list every frame's origin-relative presentation interval before anything decodes,
//! for `FrameStream` and for callers that decode regions or re-encode frames by index; list the
//! presentation indices of the packets the container flags as keyframes, for the localized
//! video's segment encode.
//! **Position:** inside `video_frames`; `FrameStream::open` and `open_native` read the timeline
//! first; `encode::segments` reads the keyframes.
//! **Signals and state:** bounded ffprobe runs, for the container origin and the video packet
//! table, which the demuxer reads without decoding; holds nothing afterwards.
//! **Invariants:** the timeline is every decodable packet's presentation time in presentation
//! order (B-frame packets arrive in decode order); a packet flagged for the decoder to discard has
//! no frame. Every time uses the common container origin, preserving the video offset relative to
//! audio. The next presentation timestamp ends a frame; only the final frame uses its reported or
//! fallback duration. A repeated or reversed time is an error. Keyframes number frames exactly as
//! the timeline does: the same discarded packets are skipped and the same stable sort orders them.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use child_process::Run;

use super::valid_rate;
use crate::{MediaError, Programs};

/// The most packets a timestamp table may hold: over 92 hours at 24 frames per second.
pub(super) const MAX_PACKETS: usize = 8_000_000;
/// The longest line a timestamp table may hold.
pub(super) const MAX_LINE_BYTES: usize = 16 * 1024;
/// ffprobe's key for a packet's presentation time.
pub(super) const PACKET_TIME: &str = "pts_time";

/// Origin-relative (time_s, end_s) of every frame of the first video stream, by presentation
/// index. `start_s` is the origin when the container reports none; `1 / fps` ends the final
/// frame when its packet reports no duration.
pub fn timeline(
    programs: &Programs,
    video: &Path,
    start_s: f64,
    fps: f64,
) -> Result<Vec<(f64, f64)>, MediaError> {
    if !start_s.is_finite() || !valid_rate(fps) {
        return Err(MediaError::Parse("invalid video timing".into()));
    }
    let origin_s = container_origin(programs, video).unwrap_or(start_s);
    packet_timeline(programs, video, origin_s, 1.0 / fps)
}

/// The format origin is shared with every stream; the caller's offset is only a fallback.
pub(super) fn container_origin(programs: &Programs, video: &Path) -> Option<f64> {
    let mut running = Run::new(&programs.ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=start_time",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(video)
        .timeout(Duration::from_secs(60))
        .spawn()
        .ok()?;
    let mut bytes = Vec::new();
    running
        .take_stdout()?
        .take(1025)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 || running.wait().ok()?.code != 0 {
        return None;
    }
    parse_origin(std::str::from_utf8(&bytes).ok()?)
}

pub(super) fn parse_origin(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|time| time.is_finite())
}

/// The presentation timeline from the first video stream's packet table, which ffprobe reads
/// from the container without decoding.
fn packet_timeline(
    programs: &Programs,
    video: &Path,
    origin_s: f64,
    fallback: f64,
) -> Result<Vec<(f64, f64)>, MediaError> {
    let table = packet_table(programs, video)?;
    let packets = parse_packets(&table, fallback, MAX_PACKETS)?;
    presentation_timeline(&packets, origin_s)
}

/// A packet of the first video stream that the container flags as a keyframe.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KeyframePacket {
    /// The frame it presents, numbered as [`timeline`] numbers frames.
    pub index: u64,
    /// Its presentation time on the container's own clock, not relative to the origin: the time
    /// a seek in this file names.
    pub pts_s: f64,
}

/// The presentation index of every frame of the first video stream whose packet carries the
/// keyframe flag, ascending; numbered as [`timeline`] numbers frames.
pub fn keyframes(programs: &Programs, video: &Path) -> Result<Vec<u64>, MediaError> {
    Ok(keyframe_packets(programs, video)?
        .into_iter()
        .map(|keyframe| keyframe.index)
        .collect())
}

/// Every keyframe packet of the first video stream with its presentation index and time,
/// ascending.
pub fn keyframe_packets(
    programs: &Programs,
    video: &Path,
) -> Result<Vec<KeyframePacket>, MediaError> {
    parse_keyframes(&packet_table(programs, video)?, MAX_PACKETS)
}

/// The keyframes of a compact packet table: the packets [`parse_packets`] keeps, in the same
/// stable presentation order, numbered, and filtered to those whose flags carry `K`.
pub(super) fn parse_keyframes(
    table: &str,
    limit: usize,
) -> Result<Vec<KeyframePacket>, MediaError> {
    let mut packets = Vec::new();
    for line in table.lines() {
        if line.len() > MAX_LINE_BYTES {
            return Err(excessive_table());
        }
        if discarded(line) {
            continue;
        }
        if let Some((time, _)) = parse_timing(line, PACKET_TIME, 0.0) {
            if packets.len() == limit {
                return Err(excessive_table());
            }
            packets.push((time, flagged_key(line)));
        }
    }
    packets.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(packets
        .into_iter()
        .enumerate()
        .filter(|(_, (_, key))| *key)
        .map(|(index, (pts_s, _))| KeyframePacket {
            index: index as u64,
            pts_s,
        })
        .collect())
}

/// Whether a packet line's flags carry `K`: the container marks the packet as a keyframe.
pub(super) fn flagged_key(line: &str) -> bool {
    line.trim().split('|').any(|part| {
        part.strip_prefix("flags=")
            .is_some_and(|flags| flags.contains('K'))
    })
}

/// The first video stream's compact packet table: presentation time, duration and flags of
/// every packet in stored order.
fn packet_table(programs: &Programs, video: &Path) -> Result<String, MediaError> {
    let output = Run::new(&programs.ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_packets",
            "-show_entries",
            "packet=pts_time,duration_time,flags",
            "-of",
            "compact=p=0",
        ])
        .arg(video)
        .timeout(Duration::from_secs(600))
        .output()?;
    if output.code != 0 {
        return Err(MediaError::Exit {
            program: programs.ffprobe.clone(),
            code: output.code,
            stderr: output.stderr,
        });
    }
    Ok(output.stdout)
}

/// The (time, duration) of every decodable packet, sorted into presentation order. A line
/// without a finite presentation time has no frame, and neither has a packet flagged for the
/// decoder to discard, such as the pre-roll an MP4 edit list skips.
pub(super) fn parse_packets(
    table: &str,
    fallback: f64,
    limit: usize,
) -> Result<Vec<(f64, f64)>, MediaError> {
    let mut packets = Vec::new();
    for line in table.lines() {
        if line.len() > MAX_LINE_BYTES {
            return Err(excessive_table());
        }
        if discarded(line) {
            continue;
        }
        if let Some(timing) = parse_timing(line, PACKET_TIME, fallback) {
            if packets.len() == limit {
                return Err(excessive_table());
            }
            packets.push(timing);
        }
    }
    packets.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(packets)
}

fn excessive_table() -> MediaError {
    MediaError::Parse("excessive frame timestamp table".into())
}

/// Whether a packet line's flags carry `D`: the demuxer marks the packet for the decoder to
/// discard, so it produces no frame.
pub(super) fn discarded(line: &str) -> bool {
    line.trim().split('|').any(|part| {
        part.strip_prefix("flags=")
            .is_some_and(|flags| flags.contains('D'))
    })
}

/// Origin-relative (start, end) of packets in presentation order: each frame ends where the next
/// begins, and the final frame after its own duration.
pub(super) fn presentation_timeline(
    packets: &[(f64, f64)],
    origin: f64,
) -> Result<Vec<(f64, f64)>, MediaError> {
    packets
        .iter()
        .enumerate()
        .map(|(i, &timing)| frame_interval(timing, packets.get(i + 1).copied(), origin))
        .collect()
}

/// A frame's origin-relative interval; a zero-length or reversed interval is an error.
pub(super) fn frame_interval(
    timing: (f64, f64),
    next: Option<(f64, f64)>,
    origin: f64,
) -> Result<(f64, f64), MediaError> {
    let (time, duration) = timing;
    let end = next.map_or(time + duration, |(time, _)| time);
    if !end.is_finite() || end <= time {
        return Err(MediaError::Parse(
            "repeated or nonmonotonic frame presentation timestamps".into(),
        ));
    }
    Ok(((time - origin).max(0.0), (end - origin).max(0.0)))
}

/// The time under `time_key` and the duration of one compact ffprobe line, if the time is
/// finite; a missing, non-positive or non-finite duration falls back.
pub(super) fn parse_timing(line: &str, time_key: &str, fallback: f64) -> Option<(f64, f64)> {
    let mut time = None;
    let mut duration = fallback;
    for part in line.trim().split('|') {
        if let Some((key, value)) = part.split_once('=') {
            if key == time_key {
                time = value.parse::<f64>().ok().filter(|v| v.is_finite());
            } else if key == "duration_time" {
                duration = value
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .unwrap_or(fallback);
            }
        }
    }
    time.map(|t| (t, duration))
}
