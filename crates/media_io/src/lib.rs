//! Media input through FFmpeg and ffprobe.
//!
//! **Role:** everything the pipeline reads from a video file: the probe result, the audio as
//! 32-bit float PCM, and the shot-change times.
//!
//! **Position:** called by `stages`; runs `ffprobe` and `ffmpeg` through `child_process` and
//! returns `job_model` types. Links no libav.
//!
//! **Signals and state:** reads the video file through child processes; writes nothing itself.
//!
//! **Invariants:** audio is streamed in fixed-size chunks through a bounded channel and never held
//! whole at 44.1 kHz; FFmpeg's stderr is drained on its own thread; the source video is only read.

pub mod pcm_stream;
pub mod probe;
pub mod shot_changes;
