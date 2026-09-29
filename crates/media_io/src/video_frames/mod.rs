//! Bounded RGB video decoding with original presentation timestamps.
//!
//! **Role:** pair FFmpeg's raw frames with ffprobe's frame timing, without retaining a video.
//! **Position:** media input for text detection, tracking and validation tools.
//! **Signals and state:** a bounded header probe, two streaming children and their stdout
//! pipes, one RGB frame and one lookahead presentation timestamp at a time.
//! **Invariants:** no frame-rate conversion; failures and truncated frames are errors; dropping
//! the stream kills both children; the source is only read. Every timestamp uses the common
//! container origin, preserving the video offset relative to audio. The next presentation
//! timestamp ends a frame; only the final frame uses its reported or fallback duration.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::ChildStdout;
use std::time::Duration;

use crate::{MediaError, Programs};
use child_process::{Run, Running};

pub struct VideoFrame {
    pub time_s: f64,
    pub end_s: f64,
    pub rgb: Vec<u8>,
}

pub struct FrameStream {
    decoder: Option<Running>,
    probe: Option<Running>,
    pixels: ChildStdout,
    times: BufReader<ChildStdout>,
    bytes: usize,
    origin_s: f64,
    next_timing: Option<(f64, f64)>,
    fallback_duration: f64,
    program: String,
}

impl FrameStream {
    pub fn open(
        programs: &Programs,
        video: &Path,
        size: (u32, u32),
        start_s: f64,
        fps: f64,
    ) -> Result<Self, MediaError> {
        let bytes = (size.0 as usize)
            .checked_mul(size.1 as usize)
            .and_then(|v| v.checked_mul(3))
            .filter(|v| *v > 0 && *v <= 256 * 1024 * 1024)
            .ok_or_else(|| MediaError::Parse("invalid or excessive video dimensions".into()))?;
        if !start_s.is_finite() || !fps.is_finite() || fps <= 0.0 || !(1.0 / fps).is_finite() {
            return Err(MediaError::Parse("invalid video timing".into()));
        }
        let origin_s = container_origin(programs, video).unwrap_or(start_s);
        let mut decoder = Run::new(&programs.ffmpeg)
            .args([
                "-nostdin",
                "-hide_banner",
                "-v",
                "error",
                "-noautorotate",
                "-i",
            ])
            .arg(video)
            .args([
                "-map",
                "0:v:0",
                "-an",
                "-sn",
                "-dn",
                "-fps_mode",
                "passthrough",
                "-vf",
            ])
            .arg(format!("scale={}:{}", size.0, size.1))
            .args(["-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"])
            .timeout(Duration::from_secs(24 * 3600))
            .spawn()?;
        let mut probe = Run::new(&programs.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_frames",
                "-show_entries",
                "frame=best_effort_timestamp_time,duration_time",
                "-of",
                "compact=p=0",
            ])
            .arg(video)
            .timeout(Duration::from_secs(24 * 3600))
            .spawn()?;
        let pixels = decoder
            .take_stdout()
            .ok_or_else(|| MediaError::Parse("no video pipe".into()))?;
        let times = BufReader::new(
            probe
                .take_stdout()
                .ok_or_else(|| MediaError::Parse("no timestamp pipe".into()))?,
        );
        Ok(Self {
            decoder: Some(decoder),
            probe: Some(probe),
            pixels,
            times,
            bytes,
            origin_s,
            next_timing: None,
            fallback_duration: 1.0 / fps,
            program: programs.ffmpeg.clone(),
        })
    }

    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>, MediaError> {
        let timing = match self.next_timing.take() {
            Some(timing) => Some(timing),
            None => read_timing(&mut self.times, self.fallback_duration)?,
        };
        if timing.is_some() {
            self.next_timing = read_timing(&mut self.times, self.fallback_duration)?;
        }
        let mut rgb = vec![0; self.bytes];
        let mut filled = 0;
        while filled < rgb.len() {
            let n = self.pixels.read(&mut rgb[filled..]).map_err(parse_io)?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        match (timing, filled) {
            (None, 0) => Ok(None),
            (Some((time, duration)), n) if n == self.bytes => {
                let (time_s, end_s) =
                    frame_interval((time, duration), self.next_timing, self.origin_s)?;
                Ok(Some(VideoFrame { time_s, end_s, rgb }))
            }
            _ => Err(MediaError::Parse(
                "decoded frames and presentation timestamps disagree".into(),
            )),
        }
    }

    pub fn finish(mut self) -> Result<(), MediaError> {
        for running in [&mut self.decoder, &mut self.probe] {
            let result = running.take().expect("running child").wait()?;
            if result.code != 0 {
                return Err(MediaError::Exit {
                    program: self.program.clone(),
                    code: result.code,
                    stderr: result.stderr,
                });
            }
        }
        Ok(())
    }
}

fn parse_io(error: std::io::Error) -> MediaError {
    MediaError::Parse(error.to_string())
}

/// The format origin is shared with every stream; the caller's offset is only a fallback.
fn container_origin(programs: &Programs, video: &Path) -> Option<f64> {
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

fn parse_origin(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|time| time.is_finite())
}

fn read_timing(reader: &mut impl BufRead, fallback: f64) -> Result<Option<(f64, f64)>, MediaError> {
    let mut line = String::new();
    loop {
        line.clear();
        let read = reader
            .take(16 * 1024 + 1)
            .read_line(&mut line)
            .map_err(parse_io)?;
        if read > 16 * 1024 {
            return Err(MediaError::Parse("excessive frame timestamp line".into()));
        }
        if read == 0 {
            return Ok(None);
        }
        if let Some(timing) = parse_timing(&line, fallback) {
            return Ok(Some(timing));
        }
    }
}

fn frame_interval(
    timing: (f64, f64),
    next: Option<(f64, f64)>,
    origin: f64,
) -> Result<(f64, f64), MediaError> {
    let (time, duration) = timing;
    let end = next.map_or(time + duration, |(time, _)| time);
    if !end.is_finite() || end < time {
        return Err(MediaError::Parse(
            "nonmonotonic frame presentation timestamps".into(),
        ));
    }
    Ok(((time - origin).max(0.0), (end - origin).max(0.0)))
}

fn parse_timing(line: &str, fallback: f64) -> Option<(f64, f64)> {
    let mut time = None;
    let mut duration = fallback;
    for part in line.trim().split('|') {
        if let Some((key, value)) = part.split_once('=') {
            match key {
                "best_effort_timestamp_time" => {
                    time = value.parse::<f64>().ok().filter(|v| v.is_finite())
                }
                "duration_time" => {
                    duration = value
                        .parse::<f64>()
                        .ok()
                        .filter(|v| v.is_finite() && *v > 0.0)
                        .unwrap_or(fallback)
                }
                _ => {}
            }
        }
    }
    time.map(|t| (t, duration))
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;
