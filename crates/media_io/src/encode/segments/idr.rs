//! Which keyframes of a video open an IDR picture, read from the keyframe packets' own bytes.
//!
//! **Role:** read the bytes of chosen keyframe packets with ffprobe and scan their NAL units for
//! an IDR slice; remember each answer.
//! **Position:** inside `encode::segments`; the planner asks it about the keyframes around each
//! run of changed frames, and the join checks ask it where a decode may start.
//! **Signals and state:** one bounded ffprobe run per batch of up to `BATCH` keyframes, each
//! seeking to just after the keyframe and reading one packet; holds the answers given so far.
//! **Invariants:** a keyframe is an IDR only when the packet read presents within half a frame of
//! the keyframe and its first slice is an IDR slice; any other packet, an unlisted frame or an
//! unreadable packet is not an IDR, so a doubtful boundary widens or falls back, never cuts. The
//! seek lands on the keyframe in a container with a keyframe index (Matroska, MP4); in one
//! without (MPEG-TS) it may land elsewhere, and the keyframe then counts as no IDR.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::ffprobe;
use super::nal::{PacketFormat, parse_hex_dump, starts_idr_picture};
use crate::video_frames::packets::KeyframePacket;
use crate::{MediaError, Programs};

/// The most keyframe packets one ffprobe run prints; a 1080p keyframe prints about 300 KB.
const BATCH: usize = 16;

/// The IDR answers for one video's keyframes.
pub struct IdrProbe {
    programs: Programs,
    video: PathBuf,
    format: PacketFormat,
    keyframes: Vec<KeyframePacket>,
    frame_s: f64,
    known: HashMap<u64, bool>,
}

impl IdrProbe {
    /// A probe of `video`'s `keyframes` (from `keyframe_packets`), whose packets store NAL units
    /// as `format` and whose frames last `frame_s` seconds.
    pub fn new(
        programs: &Programs,
        video: &Path,
        format: PacketFormat,
        keyframes: Vec<KeyframePacket>,
        frame_s: f64,
    ) -> IdrProbe {
        IdrProbe {
            programs: programs.clone(),
            video: video.to_path_buf(),
            format,
            keyframes,
            frame_s,
            known: HashMap::new(),
        }
    }

    /// For each frame, whether it is a keyframe that opens an IDR picture; frames asked before
    /// are answered from memory.
    pub fn confirm(&mut self, frames: &[u64]) -> Result<Vec<bool>, MediaError> {
        let unknown: Vec<KeyframePacket> = frames
            .iter()
            .filter(|frame| !self.known.contains_key(frame))
            .filter_map(|&frame| self.keyframe(frame))
            .collect();
        for batch in unknown.chunks(BATCH) {
            let answers = self.read_batch(batch)?;
            for (keyframe, answer) in batch.iter().zip(answers) {
                self.known.insert(keyframe.index, answer);
            }
        }
        Ok(frames
            .iter()
            .map(|frame| self.known.get(frame).copied().unwrap_or(false))
            .collect())
    }

    /// The frames confirmed so far to open an IDR picture, ascending.
    pub fn confirmed(&self) -> Vec<u64> {
        let mut frames: Vec<u64> = self
            .known
            .iter()
            .filter(|(_, idr)| **idr)
            .map(|(frame, _)| *frame)
            .collect();
        frames.sort_unstable();
        frames
    }

    fn keyframe(&self, frame: u64) -> Option<KeyframePacket> {
        let position = self
            .keyframes
            .binary_search_by_key(&frame, |keyframe| keyframe.index)
            .ok()?;
        Some(self.keyframes[position])
    }

    /// Read one packet just after each keyframe's time and answer whether it is that keyframe
    /// and opens an IDR picture.
    fn read_batch(&self, batch: &[KeyframePacket]) -> Result<Vec<bool>, MediaError> {
        let intervals: Vec<String> = batch
            .iter()
            .map(|keyframe| format!("{:.6}%+#1", keyframe.pts_s + self.frame_s / 4.0))
            .collect();
        let intervals = intervals.join(",");
        let output = ffprobe(
            &self.programs,
            &[
                "-select_streams",
                "v:0",
                "-read_intervals",
                &intervals,
                "-show_packets",
                "-show_data",
                "-show_entries",
                "packet=pts_time,flags,data",
                "-of",
                "default",
            ],
            &self.video,
        )?;
        let packets = parse_packets(&output);
        Ok(batch
            .iter()
            .map(|keyframe| {
                packets.iter().any(|packet| {
                    (packet.pts_s - keyframe.pts_s).abs() < self.frame_s / 2.0
                        && starts_idr_picture(&packet.bytes, self.format)
                })
            })
            .collect())
    }
}

/// One packet ffprobe printed with its bytes.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PrintedPacket {
    pub pts_s: f64,
    pub bytes: Vec<u8>,
}

/// The packets in ffprobe's default output of `-show_packets -show_data`: each `[PACKET]`
/// section's `pts_time` and the hex dump after `data=`. A section without a time or with an
/// unreadable dump is left out.
pub(super) fn parse_packets(output: &str) -> Vec<PrintedPacket> {
    let mut packets = Vec::new();
    let mut lines = output.lines();
    while let Some(line) = lines.next() {
        if line.trim() != "[PACKET]" {
            continue;
        }
        let mut pts_s = None;
        let mut dump = Vec::new();
        let mut in_data = false;
        for line in lines.by_ref() {
            let line = line.trim_end();
            if line == "[/PACKET]" {
                break;
            }
            if in_data && line.get(8..10) == Some(": ") {
                dump.push(line);
                continue;
            }
            in_data = false;
            if let Some(time) = line.strip_prefix("pts_time=") {
                pts_s = time.parse::<f64>().ok().filter(|time| time.is_finite());
            } else if line == "data=" {
                in_data = true;
            }
        }
        if let (Some(pts_s), Some(bytes)) = (pts_s, parse_hex_dump(dump)) {
            packets.push(PrintedPacket { pts_s, bytes });
        }
    }
    packets
}

#[cfg(test)]
#[path = "tests/idr.rs"]
mod tests;
