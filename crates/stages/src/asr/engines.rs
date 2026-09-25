//! The `inference` speech backends as [`SpeechEngine`]s.

use inference::onnx::parakeet_tdt::{self, ParakeetTdt};
use job_model::outputs::TimedWord;

use super::SpeechEngine;

impl SpeechEngine for ParakeetTdt {
    fn name(&self) -> String {
        parakeet_tdt::MODEL.to_string()
    }

    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<TimedWord>, String> {
        ParakeetTdt::transcribe(self, samples).map_err(|e| e.to_string())
    }
}

#[cfg(feature = "crispasr")]
impl SpeechEngine for inference::ggml::crispasr::Whisper {
    fn name(&self) -> String {
        inference::ggml::crispasr::Whisper::name(self).to_string()
    }

    fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<TimedWord>, String> {
        inference::ggml::crispasr::Whisper::transcribe(self, samples)
    }
}
