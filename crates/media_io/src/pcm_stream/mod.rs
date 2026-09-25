//! FFmpeg decoding audio to 32-bit float PCM through a pipe, read in fixed-size chunks.
//!
//! **Role:** start FFmpeg on one audio track of a video, decode it to interleaved `f32le` at the
//! asked rate and channel count, and hand the samples to the caller one fixed-size chunk at a
//! time through a bounded channel; `f32_file.rs` writes such a stream to a raw `.f32` file and
//! reads one back in the same chunks.
//!
//! **Position:** called by the probe-and-decode and separation stages and by the stack spike tool;
//! runs `ffmpeg` through `child_process::Run::spawn`.
//!
//! **Signals and state:** one FFmpeg child and one reader thread per stream; at most
//! [`QUEUED_CHUNKS`] chunks wait in the channel, so memory stays bounded whatever the length.
//!
//! **Invariants:** a dropped stream kills its FFmpeg; `finish` reports FFmpeg's failure instead of
//! a short stream passing as complete.

mod f32_file;

use std::io::Read;
use std::path::Path;
use std::sync::mpsc::{Receiver, sync_channel};
use std::thread::JoinHandle;
use std::time::Duration;

use child_process::{Run, Running};

use crate::{MediaError, Programs};

pub use f32_file::{F32FileReader, F32FileWriter, read_f32_range, write_f32_file};

/// Chunks allowed to wait between the reader thread and the consumer.
pub const QUEUED_CHUNKS: usize = 4;

/// The sample rate and channel count to decode to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channels: u32,
}

impl PcmFormat {
    /// Speech recognition, voice detection, alignment and sound events.
    pub const MONO_16K: PcmFormat = PcmFormat {
        sample_rate: 16_000,
        channels: 1,
    };
    /// Vocal separation.
    pub const STEREO_44K: PcmFormat = PcmFormat {
        sample_rate: 44_100,
        channels: 2,
    };
}

/// What to decode.
#[derive(Debug, Clone)]
pub struct PcmRequest<'a> {
    pub video: &'a Path,
    /// The audio track as `-map 0:a:<n>` counts.
    pub audio_position: u32,
    pub format: PcmFormat,
    /// Start and length in seconds, for an excerpt; `None` decodes the whole track.
    pub window: Option<(f64, f64)>,
    /// Frames per chunk (samples per channel).
    pub chunk_frames: usize,
    pub deadline: Duration,
}

/// A running decode.
pub struct PcmStream {
    chunks: Receiver<Result<Vec<f32>, String>>,
    reader: Option<JoinHandle<()>>,
    ffmpeg: Option<Running>,
    program: String,
}

impl PcmStream {
    /// Start FFmpeg and the reader thread.
    pub fn open(programs: &Programs, request: &PcmRequest<'_>) -> Result<PcmStream, MediaError> {
        let mut run = Run::new(&programs.ffmpeg).args(["-nostdin", "-hide_banner", "-v", "error"]);
        if let Some((start, _)) = request.window {
            run = run.arg("-ss").arg(format!("{start:.3}"));
        }
        run = run.arg("-i").arg(request.video);
        if let Some((_, length)) = request.window {
            run = run.arg("-t").arg(format!("{length:.3}"));
        }
        let mut ffmpeg = run
            .arg("-map")
            .arg(format!("0:a:{}", request.audio_position))
            .args(["-vn", "-sn", "-dn", "-ac"])
            .arg(request.format.channels.to_string())
            .arg("-ar")
            .arg(request.format.sample_rate.to_string())
            .args(["-f", "f32le", "pipe:1"])
            .timeout(request.deadline)
            .spawn()?;
        let stdout = ffmpeg
            .take_stdout()
            .ok_or_else(|| MediaError::Parse("no stdout pipe".into()))?;
        let samples = request.chunk_frames.max(1) * request.format.channels as usize;
        let (sender, chunks) = sync_channel(QUEUED_CHUNKS);
        let reader = std::thread::spawn(move || {
            let mut stdout = stdout;
            loop {
                match read_chunk(&mut stdout, samples) {
                    Ok(Some(chunk)) => {
                        if sender.send(Ok(chunk)).is_err() {
                            return;
                        }
                    }
                    Ok(None) => return,
                    Err(e) => {
                        let _ = sender.send(Err(e.to_string()));
                        return;
                    }
                }
            }
        });
        Ok(PcmStream {
            chunks,
            reader: Some(reader),
            ffmpeg: Some(ffmpeg),
            program: programs.ffmpeg.clone(),
        })
    }

    /// The next chunk of interleaved samples; the last one may be shorter.
    pub fn next_chunk(&mut self) -> Option<Result<Vec<f32>, MediaError>> {
        self.chunks
            .recv()
            .ok()
            .map(|chunk| chunk.map_err(MediaError::Parse))
    }

    /// Wait for FFmpeg and report whether it decoded the whole request.
    pub fn finish(mut self) -> Result<(), MediaError> {
        // Drain what is left so the reader thread reaches EOF and FFmpeg can exit.
        while self.chunks.recv().is_ok() {}
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        let Some(ffmpeg) = self.ffmpeg.take() else {
            return Ok(());
        };
        let finished = ffmpeg.wait()?;
        if finished.code != 0 {
            return Err(MediaError::Exit {
                program: self.program.clone(),
                code: finished.code,
                stderr: finished.stderr,
            });
        }
        Ok(())
    }
}

/// Read up to `samples` f32 values; `None` at a clean end of stream.
fn read_chunk(source: &mut impl Read, samples: usize) -> std::io::Result<Option<Vec<f32>>> {
    let mut bytes = vec![0u8; samples * 4];
    let mut filled = 0;
    while filled < bytes.len() {
        let n = source.read(&mut bytes[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    if filled == 0 {
        return Ok(None);
    }
    bytes.truncate(filled - filled % 4);
    Ok(Some(samples_from_le(&bytes)))
}

/// Little-endian bytes to f32 samples.
pub fn samples_from_le(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

#[cfg(test)]
#[path = "tests/pcm_stream.rs"]
mod tests;
