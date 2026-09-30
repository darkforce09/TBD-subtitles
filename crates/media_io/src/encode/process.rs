//! The FFmpeg encoder a caller streams raw frames into.
//!
//! **Role:** start FFmpeg with `encode_args`, accept whole frames on its stdin, and reap it with
//! the reason it failed.
//! **Position:** inside `encode`; the localized-video task writes each composited frame here.
//! **Signals and state:** one FFmpeg child with a piped stdin, a watchdog deadline and an optional
//! cancel flag; the process holds the stdin pipe, the frame size and the count of frames written.
//! **Invariants:** only whole frames of the spec's size are written; `finish` closes stdin before
//! reaping, so FFmpeg flushes and exits; a non-zero exit carries the tail of FFmpeg's stderr; a
//! dropped process kills FFmpeg. Start, write and finish from one thread, which the child's
//! lifetime is tied to.

use std::io::Write;
use std::process::ChildStdin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use child_process::{Run, Running};

use super::{EncodeSpec, encode_args};
use crate::{MediaError, Programs};

/// The most stderr bytes an exit error carries: the end, where FFmpeg says what failed.
const STDERR_TAIL_BYTES: usize = 4096;

/// A running FFmpeg encode that reads raw frames on its stdin.
pub struct EncoderProcess {
    running: Running,
    stdin: Option<ChildStdin>,
    bytes: usize,
    written: u64,
    program: String,
}

impl EncoderProcess {
    /// Start FFmpeg on `spec`, killed at `timeout` or once `cancel` is set.
    pub fn start(
        programs: &Programs,
        spec: &EncodeSpec,
        timeout: Duration,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<Self, MediaError> {
        let bytes = spec.pixel_format.frame_bytes((spec.width, spec.height))?;
        let (num, den) = spec.frame_rate;
        if num == 0 || den == 0 || !spec.video_offset_s.is_finite() {
            return Err(MediaError::Parse("invalid encode timing".into()));
        }
        if spec.output == spec.source {
            return Err(MediaError::Parse(
                "the encode would overwrite its source".into(),
            ));
        }
        let mut run = Run::new(&programs.ffmpeg)
            .args(encode_args(spec))
            .stdin_piped()
            .timeout(timeout);
        if let Some(flag) = cancel {
            run = run.cancel_on(flag);
        }
        let mut running = run.spawn()?;
        // FFmpeg writes the output file, never stdout.
        drop(running.take_stdout());
        let stdin = running
            .take_stdin()
            .ok_or_else(|| MediaError::Parse("no encoder input pipe".into()))?;
        Ok(Self {
            running,
            stdin: Some(stdin),
            bytes,
            written: 0,
            program: programs.ffmpeg.clone(),
        })
    }

    /// The byte size of one frame the encoder reads.
    pub fn frame_bytes(&self) -> usize {
        self.bytes
    }

    /// How many frames were written.
    pub fn frames_written(&self) -> u64 {
        self.written
    }

    /// Write one whole frame. A frame of the wrong size is refused; a write that fails means
    /// FFmpeg stopped reading (it failed, timed out or was cancelled), and `finish` says why.
    pub fn write_frame(&mut self, pixels: &[u8]) -> Result<(), MediaError> {
        if pixels.len() != self.bytes {
            return Err(MediaError::Parse(format!(
                "an encoder frame holds {} bytes, not {}",
                self.bytes,
                pixels.len()
            )));
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| MediaError::Parse("the encoder input is closed".into()))?;
        if let Err(error) = stdin.write_all(pixels) {
            self.stdin = None;
            return Err(MediaError::Parse(format!(
                "the encoder stopped reading after {} frames: {error}",
                self.written
            )));
        }
        self.written += 1;
        Ok(())
    }

    /// Close the input so FFmpeg finishes the file, then reap it: the frames written on a clean
    /// exit, the deadline or cancel as a run error, or the exit code with the end of its stderr.
    pub fn finish(mut self) -> Result<u64, MediaError> {
        drop(self.stdin.take());
        let finished = self.running.wait()?;
        if finished.code != 0 {
            return Err(MediaError::Exit {
                program: self.program,
                code: finished.code,
                stderr: tail(&finished.stderr, STDERR_TAIL_BYTES),
            });
        }
        Ok(self.written)
    }
}

/// The last `limit` bytes of `text`, cut at a character boundary.
fn tail(text: &str, limit: usize) -> String {
    let mut start = text.len().saturating_sub(limit);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    text[start..].to_string()
}

#[cfg(test)]
#[path = "tests/process.rs"]
mod tests;
