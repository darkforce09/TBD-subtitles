//! Speech found in a 16 kHz stem, and the chunk plan every speech engine transcribes.
//!
//! **Role:** score every 16 ms frame of a stem with earshot, turn the scores into padded and
//! merged speech regions, and cut the audio into chunks at silences.
//!
//! **Position:** called by the voice-activity stage and the stack spike tool; reads a raw `.f32`
//! stem through `media_io`; `regions.rs` and `chunk_plan.rs` hold the pure decisions.
//!
//! **Signals and state:** one earshot detector (a few KiB) and one score per frame.
//!
//! **Invariants:** earshot sees exactly 256-sample frames at 16 kHz; a short last frame is
//! padded with silence.

pub mod chunk_plan;
pub mod regions;

use std::path::Path;

use earshot::Detector;
use job_model::outputs::SpeechPlan;
use media_io::MediaError;
use media_io::pcm_stream::F32FileReader;

/// Samples per earshot frame: 16 ms at 16 kHz.
pub const FRAME: usize = 256;
/// Seconds per earshot frame.
pub const FRAME_S: f64 = FRAME as f64 / 16_000.0;

/// The settings that turn scores into regions and regions into chunks.
#[derive(Debug, Clone, Copy)]
pub struct VadSettings {
    pub threshold: f32,
    pub pad_s: f64,
    pub merge_gap_s: f64,
    pub chunks: chunk_plan::ChunkSettings,
}

impl Default for VadSettings {
    /// Pad 200 ms, merge gaps under 300 ms, chunks of 20 to 60 s cut at silences of 0.35 s.
    fn default() -> VadSettings {
        VadSettings {
            threshold: 0.5,
            pad_s: 0.2,
            merge_gap_s: 0.3,
            chunks: chunk_plan::ChunkSettings::default(),
        }
    }
}

/// One earshot score per 16 ms frame of a 16 kHz mono `.f32` file.
pub fn score_file(stem: &Path) -> Result<Vec<f32>, MediaError> {
    let mut reader = F32FileReader::open(stem, FRAME * 1024)?;
    let mut detector = Detector::default();
    let mut scores = Vec::new();
    while let Some(chunk) = reader.next_chunk()? {
        for frame in chunk.chunks(FRAME) {
            if frame.len() == FRAME {
                scores.push(detector.predict_f32(frame));
            } else {
                let mut padded = frame.to_vec();
                padded.resize(FRAME, 0.0);
                scores.push(detector.predict_f32(&padded));
            }
        }
    }
    Ok(scores)
}

/// Regions and chunks from frame scores.
pub fn plan(scores: &[f32], duration_s: f64, settings: &VadSettings) -> SpeechPlan {
    let regions = regions::from_scores(
        scores,
        FRAME_S,
        settings.threshold,
        settings.pad_s,
        settings.merge_gap_s,
        duration_s,
    );
    let chunks = chunk_plan::cut(&regions, scores, FRAME_S, &settings.chunks);
    SpeechPlan {
        frame_s: FRAME_S,
        threshold: settings.threshold,
        regions,
        chunks,
    }
}
