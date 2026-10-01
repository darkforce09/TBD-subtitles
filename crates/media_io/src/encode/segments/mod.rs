//! Re-encoding only the frames a localized video changes, and copying the rest from the source.
//!
//! **Role:** the media side of a localized video built from pieces: probe the source's H.264
//! stream and decide whether pieces can be joined to it; find its keyframes that open an IDR
//! picture; plan which frames are copied and which re-encoded; cut the copied pieces with their
//! headers in-band; describe the segment encode that matches the source; join every piece with
//! the source's audio; and check the joined file before it is installed.
//! **Position:** inside `encode`; the localize stage in `stages` drives it and renders the frames
//! of each re-encoded piece into `EncoderProcess::start_segment`.
//! **Signals and state:** bounded ffprobe and FFmpeg child processes; files only in the folder the
//! caller names and the output it names; `IdrProbe` remembers its answers.
//! **Invariants:** the source is only read; every piece starts at frame 0 or an IDR keyframe and
//! carries its stream headers before every keyframe, so a decoder crosses every join; copied
//! packets keep their bytes; a joined file is installed only after every check passes, and any
//! doubt is a [`FallbackReason`] for the whole-video encode.

mod args;
mod copied;
mod idr;
mod join;
mod nal;
mod plan;
mod profile;
mod source;
mod verify;

pub use crate::video_frames::packets::{KeyframePacket, keyframe_packets, keyframes};
pub use args::{
    NVENC_SEGMENT_CQ, NVENC_SEGMENT_PRESET, SegmentSpec, X264_SEGMENT_CRF, X264_SEGMENT_PRESET,
    copied_piece_name, copy_args, segment_args, segment_encoder,
};
pub use idr::IdrProbe;
pub use join::{
    JOIN_LIST, JoinRequest, copy_pieces, join_args, join_list, join_pieces, piece_file,
};
pub use nal::{
    NAL_IDR_SLICE, NAL_PPS, NAL_SLICE, NAL_SPS, PacketFormat, nal_unit_types, parse_hex_dump,
    starts_idr_picture,
};
pub use plan::{FrameSpan, IdrCheck, Piece, PlanError, piece_summary, plan_pieces};
pub use profile::{H264Level, H264Profile, PeakRate, peak_rate};
pub use source::{H264Source, StreamStarts, probe_h264_source, segment_eligibility, stream_starts};
pub use verify::{DECODE_LAG_S, DECODE_LEAD_S, VerifyRequest, verify_join};

use std::fmt;
use std::path::Path;
use std::time::Duration;

use child_process::Run;

use crate::{MediaError, Programs};

/// ffprobe reads headers and a few packets; a minute means something is wrong.
const PROBE_DEADLINE: Duration = Duration::from_secs(60);

/// Why the localized video is encoded whole instead of from pieces: the report shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackReason(pub String);

impl fmt::Display for FallbackReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FallbackReason {}

/// Run ffprobe printing only errors, with `args` and then `video`; its stdout, or the exit as an
/// error.
fn ffprobe(programs: &Programs, args: &[&str], video: &Path) -> Result<String, MediaError> {
    let output = Run::new(&programs.ffprobe)
        .args(["-v", "error"])
        .args(args)
        .arg(video)
        .timeout(PROBE_DEADLINE)
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

#[cfg(test)]
#[path = "tests/segments.rs"]
mod tests;
