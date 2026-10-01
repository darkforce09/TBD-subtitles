//! What a source's H.264 stream is made of, as far as a segment must match it, and whether
//! segments can be joined to it at all.
//!
//! **Role:** read the first video stream's codec, profile, level, reference frames, reorder depth,
//! pixel format, size, sample aspect ratio, colour tags, frame rates, field order, packet format
//! and bit rate with one ffprobe run; read the first video and first audio timestamps; decide
//! whether the source is one segments can be joined to, and say why not.
//! **Position:** inside `encode::segments`; the localize stage probes its source here before
//! planning, and the join checks probe the joined output the same way.
//! **Signals and state:** bounded ffprobe runs; holds nothing.
//! **Invariants:** a value ffprobe does not report stays unknown, never guessed; only a
//! progressive, constant-frame-rate H.264 4:2:0 stream that starts on a keyframe is eligible.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::nal::PacketFormat;
use super::profile::{H264Level, H264Profile};
use super::{FallbackReason, ffprobe};
use crate::encode::{VideoColour, is_constant_frame_rate};
use crate::{MediaError, Programs};

/// How many leading video packets hold the first frame to present: more than the deepest
/// reordering H.264 allows (16 frames).
const LEADING_PACKETS: u32 = 64;
/// How far the average frame rate may stray from the nominal one before the source counts as
/// variable, as a share of the nominal rate.
const AVERAGE_RATE_TOLERANCE: f64 = 0.001;

/// The first video stream of a source, as a segment encoder must match it.
#[derive(Debug, Clone, PartialEq)]
pub struct H264Source {
    /// ffprobe's codec name, `h264` for H.264.
    pub codec: String,
    /// The profile as ffprobe names it, such as `High`.
    pub profile_name: String,
    /// The profile a segment can match; `None` for any other.
    pub profile: Option<H264Profile>,
    pub level: H264Level,
    /// The most reference frames a picture may use (`max_num_ref_frames`).
    pub refs: u32,
    /// How many frames the decoder holds back to reorder B-frames; zero without B-frames.
    pub reorder_depth: u32,
    pub pix_fmt: Option<String>,
    pub width: u32,
    pub height: u32,
    /// The sample aspect ratio, when the stream states one.
    pub sample_aspect_ratio: Option<(u32, u32)>,
    pub colour: VideoColour,
    /// The nominal frame rate (`r_frame_rate`) as a fraction.
    pub frame_rate: (u32, u32),
    /// The average frame rate over the stream (`avg_frame_rate`); (0, 0) when unknown.
    pub average_frame_rate: (u32, u32),
    /// `progressive`, or how the fields are ordered; `None` when unknown.
    pub field_order: Option<String>,
    /// How packets store their NAL units; `None` when ffprobe does not say.
    pub packet_format: Option<PacketFormat>,
    /// The video's bit rate: the stream's, else Matroska's `BPS` statistic, else the whole
    /// file's.
    pub bit_rate: Option<u64>,
    pub starts: StreamStarts,
}

impl H264Source {
    /// The nominal frame rate in frames per second; zero when unknown.
    pub fn fps(&self) -> f64 {
        rate(self.frame_rate)
    }

    /// One frame's duration in seconds; zero when the rate is unknown.
    pub fn frame_s(&self) -> f64 {
        let fps = self.fps();
        if fps > 0.0 { 1.0 / fps } else { 0.0 }
    }
}

/// When the first frame of video and the first packet of audio present, on the file's own clock.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StreamStarts {
    /// The earliest presentation time of a decodable packet of the first video stream.
    pub video_s: Option<f64>,
    /// The presentation time of the first packet of the first audio stream, discarded or not:
    /// copying the audio keeps every packet's place.
    pub audio_s: Option<f64>,
}

impl StreamStarts {
    /// How long after the audio the video starts; negative when it starts before; `None`
    /// without both.
    pub fn video_after_audio_s(&self) -> Option<f64> {
        Some(self.video_s? - self.audio_s?)
    }
}

/// Probe the first video stream of `video` and when its video and audio start.
pub fn probe_h264_source(programs: &Programs, video: &Path) -> Result<H264Source, MediaError> {
    let output = ffprobe(
        programs,
        &[
            "-select_streams",
            "v:0",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ],
        video,
    )?;
    let mut source = parse_source(&output)?;
    source.starts = stream_starts(programs, video)?;
    Ok(source)
}

/// When the first video frame and the first audio packet of `video` present.
pub fn stream_starts(programs: &Programs, video: &Path) -> Result<StreamStarts, MediaError> {
    let leading = format!("%+#{LEADING_PACKETS}");
    let video_table = ffprobe(programs, &packet_args("v:0", &leading), video)?;
    let audio_table = ffprobe(programs, &packet_args("a:0", "%+#1"), video)?;
    Ok(StreamStarts {
        video_s: first_presentation(&video_table, true),
        audio_s: first_presentation(&audio_table, false),
    })
}

/// ffprobe's arguments for the leading packets `interval` reads of `stream`.
fn packet_args<'a>(stream: &'a str, interval: &'a str) -> [&'a str; 9] {
    [
        "-select_streams",
        stream,
        "-read_intervals",
        interval,
        "-show_packets",
        "-show_entries",
        "packet=pts_time,flags",
        "-of",
        "compact=p=0",
    ]
}

/// The earliest presentation time in a compact packet table, skipping packets flagged for the
/// decoder to discard when `decodable_only`.
pub(super) fn first_presentation(table: &str, decodable_only: bool) -> Option<f64> {
    table
        .lines()
        .filter(|line| !decodable_only || !discard_flagged(line))
        .filter_map(|line| field(line, "pts_time")?.parse::<f64>().ok())
        .filter(|time| time.is_finite())
        .min_by(f64::total_cmp)
}

fn discard_flagged(line: &str) -> bool {
    field(line, "flags").is_some_and(|flags| flags.contains('D'))
}

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.trim()
        .split('|')
        .find_map(|part| part.strip_prefix(key)?.strip_prefix('='))
}

/// Whether segments can be joined to `source`: H.264 in a profile a segment can match, 4:2:0 in
/// 8 or 10 bits, progressive, a known level and packet format, an even size, a constant frame
/// rate over `timeline`, and a first frame that `keyframes` lists. The reason when not.
pub fn segment_eligibility(
    source: &H264Source,
    timeline: &[(f64, f64)],
    keyframes: &[u64],
) -> Result<(), FallbackReason> {
    let refuse = |reason: String| Err(FallbackReason(reason));
    if source.codec != "h264" {
        return refuse(format!("the video is {}, not H.264", source.codec));
    }
    let Some(profile) = source.profile else {
        return refuse(format!(
            "the H.264 profile {} has no segment encoder",
            source.profile_name
        ));
    };
    let expected = if profile.is_ten_bit() {
        "yuv420p10le"
    } else {
        "yuv420p"
    };
    if source.pix_fmt.as_deref() != Some(expected) {
        return refuse(format!(
            "the pixel format {} is not {expected}",
            source.pix_fmt.as_deref().unwrap_or("unknown")
        ));
    }
    if source
        .field_order
        .as_deref()
        .is_some_and(|order| order != "progressive")
    {
        return refuse("the video is interlaced".into());
    }
    if !source.level.is_known() {
        return refuse(format!("the H.264 level {} is unknown", source.level.0));
    }
    if source.packet_format.is_none() {
        return refuse("the H.264 packet format is unknown".into());
    }
    if source.width == 0 || source.height == 0 || source.width % 2 + source.height % 2 != 0 {
        return refuse(format!(
            "the frame size {}x{} is not even",
            source.width, source.height
        ));
    }
    let fps = source.fps();
    let average = rate(source.average_frame_rate);
    if fps <= 0.0
        || (average > 0.0 && (average - fps).abs() > fps * AVERAGE_RATE_TOLERANCE)
        || !is_constant_frame_rate(timeline, fps)
    {
        return refuse("the video has a variable frame rate".into());
    }
    if keyframes.first() != Some(&0) {
        return refuse("the video does not start on a keyframe".into());
    }
    Ok(())
}

/// Read ffprobe's JSON for the first video stream and the format.
pub(super) fn parse_source(json: &str) -> Result<H264Source, MediaError> {
    let raw: RawProbe = serde_json::from_str(json).map_err(|e| MediaError::Parse(e.to_string()))?;
    let stream = raw
        .streams
        .into_iter()
        .next()
        .ok_or_else(|| MediaError::Parse("the file has no video stream".into()))?;
    let profile_name = stream.profile.clone().unwrap_or_default();
    let tag_rate = |key: &str| stream.tags.get(key).and_then(|rate| rate.parse().ok());
    let bit_rate = stream
        .bit_rate
        .as_deref()
        .and_then(|rate| rate.parse().ok())
        .or_else(|| tag_rate("BPS"))
        .or_else(|| tag_rate("BPS-eng"))
        .or_else(|| raw.format.and_then(|f| f.bit_rate?.parse().ok()))
        .filter(|&rate: &u64| rate > 0);
    let packet_format = match stream.is_avc.as_deref() {
        Some("true") => stream
            .nal_length_size
            .as_deref()
            .and_then(|size| size.parse::<u8>().ok())
            .filter(|size| (1..=4).contains(size))
            .map(PacketFormat::LengthPrefixed),
        Some("false") => Some(PacketFormat::AnnexB),
        _ => None,
    };
    Ok(H264Source {
        codec: stream.codec_name.unwrap_or_default(),
        profile: H264Profile::from_ffprobe(&profile_name),
        profile_name,
        level: H264Level(u32::try_from(stream.level.unwrap_or(0)).unwrap_or(0)),
        refs: stream.refs.unwrap_or(1),
        reorder_depth: stream.has_b_frames.unwrap_or(0),
        pix_fmt: known(stream.pix_fmt),
        width: stream.width.unwrap_or(0),
        height: stream.height.unwrap_or(0),
        sample_aspect_ratio: stream
            .sample_aspect_ratio
            .as_deref()
            .map(|text| fraction(text, ':'))
            .filter(|&(num, den)| num > 0 && den > 0),
        colour: VideoColour {
            primaries: known(stream.color_primaries),
            transfer: known(stream.color_transfer),
            matrix: known(stream.color_space),
            range: known(stream.color_range),
        },
        frame_rate: fraction(stream.r_frame_rate.as_deref().unwrap_or("0/0"), '/'),
        average_frame_rate: fraction(stream.avg_frame_rate.as_deref().unwrap_or("0/0"), '/'),
        field_order: known(stream.field_order),
        packet_format,
        bit_rate,
        starts: StreamStarts::default(),
    })
}

/// A rate fraction in frames per second; zero for a zero numerator or denominator.
fn rate((num, den): (u32, u32)) -> f64 {
    if num == 0 || den == 0 {
        0.0
    } else {
        f64::from(num) / f64::from(den)
    }
}

fn fraction(text: &str, separator: char) -> (u32, u32) {
    let mut parts = text.split(separator);
    let num = parts
        .next()
        .and_then(|n| n.trim().parse().ok())
        .unwrap_or(0);
    let den = parts
        .next()
        .and_then(|d| d.trim().parse().ok())
        .unwrap_or(0);
    (num, den)
}

/// A value ffprobe knows: its placeholders for an unset value are no value.
fn known(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !matches!(value.as_str(), "" | "unknown" | "unspecified" | "reserved"))
}

#[derive(Deserialize)]
struct RawProbe {
    #[serde(default)]
    streams: Vec<RawStream>,
    format: Option<RawFormat>,
}

#[derive(Deserialize)]
struct RawStream {
    codec_name: Option<String>,
    profile: Option<String>,
    level: Option<i64>,
    refs: Option<u32>,
    has_b_frames: Option<u32>,
    pix_fmt: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sample_aspect_ratio: Option<String>,
    color_primaries: Option<String>,
    color_transfer: Option<String>,
    color_space: Option<String>,
    color_range: Option<String>,
    r_frame_rate: Option<String>,
    avg_frame_rate: Option<String>,
    field_order: Option<String>,
    is_avc: Option<String>,
    nal_length_size: Option<String>,
    bit_rate: Option<String>,
    #[serde(default)]
    tags: HashMap<String, String>,
}

#[derive(Deserialize)]
struct RawFormat {
    bit_rate: Option<String>,
}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;
