//! Media input through FFmpeg and ffprobe.
//!
//! **Role:** everything the pipeline reads from a video file: the probe result, the audio as
//! 32-bit float PCM, and the shot-change times.
//!
//! **Position:** called by `stages` and the stack spike tool; runs `ffprobe` and `ffmpeg` through
//! `child_process` and returns `job_model` types. Links no libav.
//!
//! **Signals and state:** reads the video file through child processes; writes nothing itself.
//!
//! **Invariants:** audio is streamed in fixed-size chunks through a bounded channel and never held
//! whole at 44.1 kHz; FFmpeg's stderr is drained on its own thread; the source video is only read.

pub mod pcm_stream;
pub mod probe;
pub mod shot_changes;

use std::fmt;

use child_process::RunError;

/// The FFmpeg programs to run: `ffmpeg` and `ffprobe` on the `PATH` unless named otherwise.
#[derive(Debug, Clone)]
pub struct Programs {
    pub ffmpeg: String,
    pub ffprobe: String,
}

impl Default for Programs {
    fn default() -> Programs {
        Programs {
            ffmpeg: "ffmpeg".to_string(),
            ffprobe: "ffprobe".to_string(),
        }
    }
}

/// Why reading a video failed.
#[derive(Debug)]
pub enum MediaError {
    /// The program could not be run, was killed, or timed out.
    Run(RunError),
    /// The program ran and failed.
    Exit {
        program: String,
        code: i32,
        stderr: String,
    },
    /// The program's output could not be read.
    Parse(String),
    /// The video has no audio stream to decode.
    NoAudio,
    /// Several untagged audio tracks and none marked English.
    AmbiguousAudio(usize),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaError::Run(e) => write!(f, "{e}"),
            MediaError::Exit {
                program,
                code,
                stderr,
            } => write!(f, "{program} exited {code}: {}", stderr.trim()),
            MediaError::Parse(message) => write!(f, "unreadable output: {message}"),
            MediaError::NoAudio => write!(f, "the video has no audio stream"),
            MediaError::AmbiguousAudio(n) => {
                write!(f, "{n} audio streams and none tagged English; choose one")
            }
        }
    }
}

impl std::error::Error for MediaError {}

impl From<RunError> for MediaError {
    fn from(e: RunError) -> MediaError {
        MediaError::Run(e)
    }
}
