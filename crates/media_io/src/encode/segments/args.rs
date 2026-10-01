//! FFmpeg's command lines for the copied pieces and for one re-encoded segment.
//!
//! **Role:** split the source's video, unchanged, into Matroska pieces at the plan's boundaries
//! with its stream headers repeated before every keyframe; describe a segment encode that matches
//! the source's H.264 stream (profile, level, reference frames, B-frames, pixel format, size,
//! sample aspect ratio, colour tags) with its own headers in-band, closed GOPs and an IDR first
//! frame, its peak rate capped; and choose the segment encoder this FFmpeg can run.
//! **Position:** inside `encode::segments`; `copy_pieces` runs the copy command and
//! `EncoderProcess::start_segment` the segment command.
//! **Signals and state:** `segment_encoder` runs one bounded FFmpeg probe for NVENC; the rest
//! builds arguments and holds nothing.
//! **Invariants:** copied packets keep their bytes, gaining only the SPS and PPS before each
//! keyframe; a segment never reorders deeper than the source (no B-pyramid), uses no more
//! reference frames than the source, and writes headers before every keyframe, whichever
//! encoder runs; a 10-bit source is always encoded by x264.

use std::path::{Path, PathBuf};
use std::time::Duration;

use child_process::Run;
use job_model::onscreen::LocalizedEncoder;

use super::FallbackReason;
use super::profile::{H264Level, H264Profile, PeakRate, peak_rate};
use super::source::H264Source;
use crate::encode::{VideoColour, colour_args, path, strings};
use crate::video_frames::PixelFormat;
use crate::{MediaError, Programs};

/// x264's preset for segments: the encode-bench measured `veryfast` within 0.1 dB of `slow`, in
/// files no larger, twice as fast.
pub const X264_SEGMENT_PRESET: &str = "veryfast";
/// x264's constant rate factor for segments, as for the whole-video x264 encode.
pub const X264_SEGMENT_CRF: u32 = 16;
/// NVENC's preset for segments: the encode-bench measured `p4` within 0.2 dB of `p7`, 1.6 times
/// as fast.
pub const NVENC_SEGMENT_PRESET: &str = "p4";
/// NVENC's constant quality for segments, as for the whole-video NVENC encode.
pub const NVENC_SEGMENT_CQ: u32 = 19;
/// B-frames between reference frames when the source has B-frames.
const SEGMENT_B_FRAMES: u32 = 3;
/// The deadline for the one-frame NVENC test encode.
const PROBE_DEADLINE: Duration = Duration::from_secs(60);

/// Everything one segment encode needs: raw frames on stdin in the source's size, rate and
/// pixel format, encoded into a Matroska file that joins the source's own stream.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentSpec {
    pub width: u32,
    pub height: u32,
    /// The constant frame rate as a fraction (numerator, denominator).
    pub frame_rate: (u32, u32),
    /// The layout of the raw frames written to the encoder, which is also the encoded one.
    pub pixel_format: PixelFormat,
    /// The encoder that runs: x264 for a 10-bit source whatever was asked.
    pub encoder: LocalizedEncoder,
    /// The encoder's preset; `X264_SEGMENT_PRESET` or `NVENC_SEGMENT_PRESET` unless a bench
    /// sets another.
    pub preset: String,
    pub profile: H264Profile,
    pub level: H264Level,
    /// Reference frames, as many as the source's.
    pub refs: u32,
    /// B-frames between reference frames; zero when the source has none.
    pub b_frames: u32,
    pub sample_aspect_ratio: Option<(u32, u32)>,
    pub colour: VideoColour,
    pub peak: PeakRate,
    pub output: PathBuf,
}

impl SegmentSpec {
    /// The segment encode for an eligible `source` whose frames arrive as `pixel_format`, with
    /// the `requested` encoder (x264 for a 10-bit source), writing `output`. The reason when the
    /// source has no matching profile or level, or the frames do not match its bit depth.
    pub fn for_source(
        source: &H264Source,
        pixel_format: PixelFormat,
        requested: LocalizedEncoder,
        output: PathBuf,
    ) -> Result<SegmentSpec, FallbackReason> {
        let profile = source.profile.ok_or_else(|| {
            FallbackReason(format!(
                "the H.264 profile {} has no segment encoder",
                source.profile_name
            ))
        })?;
        let expected = if profile.is_ten_bit() {
            PixelFormat::Yuv420p10le
        } else {
            PixelFormat::Yuv420p
        };
        if pixel_format != expected {
            return Err(FallbackReason(format!(
                "{} segments take {} frames, not {}",
                source.profile_name,
                expected.name(),
                pixel_format.name()
            )));
        }
        let peak = peak_rate(source.bit_rate, profile, source.level).ok_or_else(|| {
            FallbackReason(format!("the H.264 level {} is unknown", source.level.0))
        })?;
        let encoder = match profile.nvenc_name() {
            Some(_) => requested,
            None => LocalizedEncoder::X264,
        };
        let has_b_frames = source.reorder_depth > 0 && profile != H264Profile::Baseline;
        Ok(SegmentSpec {
            width: source.width,
            height: source.height,
            frame_rate: source.frame_rate,
            pixel_format,
            encoder,
            preset: match encoder {
                LocalizedEncoder::X264 => X264_SEGMENT_PRESET,
                LocalizedEncoder::Nvenc => NVENC_SEGMENT_PRESET,
            }
            .to_string(),
            profile,
            level: source.level,
            refs: source.refs.max(1),
            b_frames: if has_b_frames { SEGMENT_B_FRAMES } else { 0 },
            sample_aspect_ratio: source.sample_aspect_ratio,
            colour: source.colour.clone(),
            peak,
            output,
        })
    }
}

/// The segment encoder to use: NVENC only when asked, the source is not 10-bit, and a one-frame
/// `h264_nvenc` test encode succeeds; x264 otherwise.
pub fn segment_encoder(
    programs: &Programs,
    requested: LocalizedEncoder,
    source: &H264Source,
) -> LocalizedEncoder {
    let ten_bit = source.profile.is_none_or(H264Profile::is_ten_bit);
    if requested == LocalizedEncoder::Nvenc && !ten_bit && h264_nvenc_runs(programs) {
        LocalizedEncoder::Nvenc
    } else {
        LocalizedEncoder::X264
    }
}

/// Whether one frame encodes with `h264_nvenc`; any failure to run or a non-zero exit is a no.
fn h264_nvenc_runs(programs: &Programs) -> bool {
    Run::new(&programs.ffmpeg)
        .args(strings(&[
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=size=256x256:rate=24",
            "-frames:v",
            "1",
            "-c:v",
            "h264_nvenc",
            "-f",
            "null",
            "-",
        ]))
        .timeout(PROBE_DEADLINE)
        .output()
        .is_ok_and(|output| output.code == 0)
}

/// The name of copied piece `number`'s file in the folder `pattern` names; the copy command
/// writes `copy00000.mkv`, `copy00001.mkv` and so on.
pub fn copied_piece_name(number: usize) -> String {
    format!("copy{number:05}.mkv")
}

/// The copy command's output pattern in `folder`.
fn copied_piece_pattern(folder: &Path) -> PathBuf {
    folder.join("copy%05d.mkv")
}

/// FFmpeg's arguments to split the source's first video stream, unchanged, into Matroska pieces
/// in `folder`, one starting at each of `boundaries` (presentation indices of IDR keyframes,
/// ascending, without 0). The stream headers go in-band before every keyframe: Annex B first,
/// which also repeats them before each IDR, then `dump_extra` for any other keyframe.
pub fn copy_args(
    source: &Path,
    boundaries: &[u64],
    frame_count: u64,
    folder: &Path,
) -> Vec<String> {
    // The segment muxer cuts every two seconds when given no cut; a cut at the frame count is
    // never reached.
    let cuts: Vec<String> = if boundaries.is_empty() {
        vec![frame_count.to_string()]
    } else {
        boundaries.iter().map(u64::to_string).collect()
    };
    let mut args = strings(&["-nostdin", "-hide_banner", "-v", "error", "-y", "-i"]);
    args.push(path(source));
    args.extend(strings(&[
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-c",
        "copy",
        "-bsf:v",
        "h264_mp4toannexb,dump_extra=freq=keyframe",
        "-f",
        "segment",
        "-segment_format",
        "matroska",
        "-segment_frames",
    ]));
    args.push(cuts.join(","));
    args.push(path(&copied_piece_pattern(folder)));
    args
}

/// FFmpeg's arguments for one segment encode: raw frames on stdin at the source's constant
/// rate, encoded alone (no audio) into Matroska.
pub fn segment_args(spec: &SegmentSpec) -> Vec<String> {
    let mut args = strings(&["-hide_banner", "-v", "error", "-y", "-f", "rawvideo"]);
    args.extend(["-pix_fmt".into(), spec.pixel_format.name().into()]);
    args.extend(["-s".into(), format!("{}x{}", spec.width, spec.height)]);
    let (num, den) = spec.frame_rate;
    args.extend(["-framerate".into(), format!("{num}/{den}")]);
    args.extend(strings(&[
        "-i", "pipe:0", "-map", "0:v:0", "-an", "-sn", "-dn",
    ]));
    if let Some((num, den)) = spec.sample_aspect_ratio {
        args.extend(["-vf".into(), format!("setsar={num}/{den}")]);
    }
    args.extend(match spec.encoder {
        LocalizedEncoder::X264 => x264_args(spec),
        LocalizedEncoder::Nvenc => nvenc_args(spec),
    });
    args.extend([
        "-maxrate".into(),
        spec.peak.max_bits_per_s.to_string(),
        "-bufsize".into(),
        spec.peak.buffer_bits.to_string(),
        "-pix_fmt".into(),
        spec.pixel_format.name().into(),
    ]);
    args.extend(colour_args(&spec.colour));
    args.extend(strings(&["-f", "matroska"]));
    args.push(path(&spec.output));
    args
}

/// x264 at constant rate factor, stitchable (headers that do not depend on the content) with
/// the headers repeated before every keyframe, closed GOPs, no B-pyramid, and the source's
/// profile, level and reference count.
fn x264_args(spec: &SegmentSpec) -> Vec<String> {
    let params = format!(
        "stitchable=1:repeat-headers=1:open-gop=0:ref={}:bframes={}:b-pyramid=none",
        spec.refs, spec.b_frames
    );
    vec![
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        spec.preset.clone(),
        "-crf".into(),
        X264_SEGMENT_CRF.to_string(),
        "-profile:v".into(),
        spec.profile.x264_name().into(),
        "-level".into(),
        spec.level.name(),
        "-x264-params".into(),
        params,
        "-forced-idr".into(),
        "1".into(),
    ]
}

/// NVENC at constant quality with the high-quality tuning, B-frames that are never references
/// (no pyramid), the source's profile, level and reference count, IDR keyframes, and the headers
/// put before every keyframe by `dump_extra`, since `h264_nvenc` has no option to repeat them.
fn nvenc_args(spec: &SegmentSpec) -> Vec<String> {
    vec![
        "-c:v".into(),
        "h264_nvenc".into(),
        "-preset".into(),
        spec.preset.clone(),
        "-tune".into(),
        "hq".into(),
        "-rc".into(),
        "vbr".into(),
        "-cq".into(),
        NVENC_SEGMENT_CQ.to_string(),
        "-b:v".into(),
        "0".into(),
        "-profile:v".into(),
        spec.profile.nvenc_name().unwrap_or("high").into(),
        "-level".into(),
        spec.level.name(),
        "-bf".into(),
        spec.b_frames.to_string(),
        "-b_ref_mode".into(),
        "disabled".into(),
        "-refs".into(),
        spec.refs.to_string(),
        "-forced-idr".into(),
        "1".into(),
        "-bsf:v".into(),
        "dump_extra=freq=keyframe".into(),
    ]
}

/// Delete `file` if it exists; any other failure is an error.
pub(super) fn remove_if_present(file: &Path) -> Result<(), MediaError> {
    match std::fs::remove_file(file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(MediaError::Parse(format!(
            "cannot remove {}: {error}",
            file.display()
        ))),
    }
}

#[cfg(test)]
#[path = "tests/args.rs"]
mod tests;
