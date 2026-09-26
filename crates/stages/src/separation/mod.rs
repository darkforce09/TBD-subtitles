//! Split the mix into a vocal stem and a background stem.
//!
//! **Role:** stream the video's audio at 44.1 kHz stereo from FFmpeg through a separation model
//! with overlap-add, and write two 16 kHz mono stems: the vocals, and the background (the mix
//! minus the vocals).
//!
//! **Position:** called inside the separation worker process (and by the stack spike tool); uses
//! `media_io::pcm_stream` for the audio and any `inference::onnx::separation::WindowModel`.
//!
//! **Signals and state:** one FFmpeg child, the model's overlap-add state and two resamplers; a
//! few model windows of audio at most, never the whole track.
//!
//! **Invariants:** both stems are written whole or not at all (each goes through a `.part` file);
//! the background is exactly the mix minus the vocals before resampling.

pub mod resample;

use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use inference::onnx::OnnxError;
use inference::onnx::separation::{CHANNELS, OverlapAdd, Separated, WindowModel};
use media_io::pcm_stream::{F32FileWriter, PcmFormat, PcmRequest, PcmStream};
use media_io::{MediaError, Programs};

use resample::Resampler;

/// Frames read from FFmpeg per chunk: one second at 44.1 kHz.
const CHUNK_FRAMES: usize = 44_100;

/// What to separate and where the stems go.
pub struct SeparationRequest<'a> {
    pub programs: &'a Programs,
    pub video: &'a Path,
    pub audio_position: u32,
    pub deadline: Duration,
    pub vocals_16k: &'a Path,
    pub background_16k: &'a Path,
    /// The track's length, the total that `progress` counts towards.
    pub duration_s: f64,
    /// Hears `(seconds separated, seconds)`.
    pub progress: &'a dyn Fn(usize, usize),
}

/// What a separation run did.
#[derive(Debug, Clone, Default)]
pub struct SeparationSummary {
    pub frames_44k: u64,
    pub samples_16k: u64,
    /// Seconds spent waiting on FFmpeg for audio.
    pub decode_s: f64,
    /// Seconds spent in overlap-add, including the model.
    pub separate_s: f64,
}

/// Why separation failed.
#[derive(Debug)]
pub enum SeparationError {
    Media(MediaError),
    Model(OnnxError),
}

impl fmt::Display for SeparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeparationError::Media(e) => write!(f, "{e}"),
            SeparationError::Model(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for SeparationError {}

impl From<MediaError> for SeparationError {
    fn from(e: MediaError) -> SeparationError {
        SeparationError::Media(e)
    }
}

impl From<OnnxError> for SeparationError {
    fn from(e: OnnxError) -> SeparationError {
        SeparationError::Model(e)
    }
}

/// The two 16 kHz mono stems being written.
struct Stems {
    vocals: (Resampler, F32FileWriter),
    background: (Resampler, F32FileWriter),
}

impl Stems {
    fn write(&mut self, separated: &Separated) -> Result<(), MediaError> {
        let frames = separated.vocals.len() / CHANNELS;
        let mut vocals = Vec::with_capacity(frames);
        let mut background = Vec::with_capacity(frames);
        for f in 0..frames {
            let (mut v, mut m) = (0f32, 0f32);
            for c in 0..CHANNELS {
                v += separated.vocals[f * CHANNELS + c];
                m += separated.mix[f * CHANNELS + c];
            }
            vocals.push(v / CHANNELS as f32);
            background.push((m - v) / CHANNELS as f32);
        }
        let out = self.vocals.0.push(&vocals);
        self.vocals.1.write(&out)?;
        let out = self.background.0.push(&background);
        self.background.1.write(&out)
    }

    fn finish(self) -> Result<u64, MediaError> {
        let (resampler, mut writer) = self.vocals;
        writer.write(&resampler.finish())?;
        let samples = writer.finish()?;
        let (resampler, mut writer) = self.background;
        writer.write(&resampler.finish())?;
        writer.finish()?;
        Ok(samples)
    }
}

/// Separate the track with `model`; returns the summary and the model for its counters.
pub fn separate<M: WindowModel>(
    model: M,
    request: &SeparationRequest<'_>,
) -> Result<(SeparationSummary, M), SeparationError> {
    let mut summary = SeparationSummary::default();
    let mut stream = PcmStream::open(
        request.programs,
        &PcmRequest {
            video: request.video,
            audio_position: request.audio_position,
            format: PcmFormat::STEREO_44K,
            window: None,
            chunk_frames: CHUNK_FRAMES,
            deadline: request.deadline,
        },
    )?;
    let mut stems = Stems {
        vocals: (Resampler::new(), F32FileWriter::create(request.vocals_16k)?),
        background: (
            Resampler::new(),
            F32FileWriter::create(request.background_16k)?,
        ),
    };
    let mut driver = OverlapAdd::new(model);
    loop {
        let waited = Instant::now();
        let Some(chunk) = stream.next_chunk() else {
            break;
        };
        let chunk = chunk?;
        summary.decode_s += waited.elapsed().as_secs_f64();
        summary.frames_44k += (chunk.len() / CHANNELS) as u64;
        let started = Instant::now();
        let separated = driver.push(&chunk)?;
        summary.separate_s += started.elapsed().as_secs_f64();
        stems.write(&separated)?;
        let total_s = request.duration_s.max(0.0).ceil() as usize;
        (request.progress)(
            ((summary.frames_44k / CHUNK_FRAMES as u64) as usize).min(total_s),
            total_s,
        );
    }
    stream.finish()?;
    let started = Instant::now();
    let (rest, model) = driver.finish()?;
    summary.separate_s += started.elapsed().as_secs_f64();
    stems.write(&rest)?;
    summary.samples_16k = stems.finish()?;
    Ok((summary, model))
}
