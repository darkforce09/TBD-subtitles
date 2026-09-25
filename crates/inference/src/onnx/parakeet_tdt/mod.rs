//! NVIDIA Parakeet-TDT-0.6B-v2 through the parakeet-rs crate: English words with times.
//!
//! **Role:** load the ONNX encoder and TDT decoder once and transcribe 16 kHz mono chunks into
//! punctuated, cased words, each timed from the model's token durations.
//!
//! **Position:** used by the speech recognition stage and the stack spike tool; parakeet-rs shares
//! this crate's `ort` and the ONNX Runtime loaded at run time.
//!
//! **Signals and state:** one parakeet-rs model with its ONNX sessions.
//!
//! **Invariants:** word times are relative to the chunk handed in; the model gives no per-word
//! probability, so confidence is `None`.

use std::path::Path;

use job_model::outputs::TimedWord;
use parakeet_rs::{ExecutionConfig, ExecutionProvider, ParakeetTDT, TimestampMode, Transcriber};

use crate::onnx::{Device, OnnxError};

/// The model id and folder name.
pub const MODEL: &str = "parakeet-tdt-0.6b-v2";

/// An opened Parakeet-TDT model.
pub struct ParakeetTdt {
    model: ParakeetTDT,
}

impl ParakeetTdt {
    /// Open the model folder (`encoder-model.onnx`, `decoder_joint-model.onnx`, `vocab.txt`).
    pub fn open(dir: &Path, device: Device) -> Result<ParakeetTdt, OnnxError> {
        let provider = match device {
            Device::Cuda => ExecutionProvider::Cuda,
            Device::Cpu => ExecutionProvider::Cpu,
        };
        let config = ExecutionConfig::new().with_execution_provider(provider);
        let model = ParakeetTDT::from_pretrained(dir, Some(config))
            .map_err(|e| OnnxError::new(format!("opening {}", dir.display()), e))?;
        Ok(ParakeetTdt { model })
    }

    /// The words of one 16 kHz mono chunk, timed from the chunk's start.
    pub fn transcribe(&mut self, samples: &[f32]) -> Result<Vec<TimedWord>, OnnxError> {
        let result = self
            .model
            .transcribe_samples(samples.to_vec(), 16_000, 1, Some(TimestampMode::Words))
            .map_err(|e| OnnxError::new("transcribing with Parakeet-TDT", e))?;
        let words = result
            .tokens
            .into_iter()
            .filter(|t| !t.text.trim().is_empty())
            .map(|t| TimedWord {
                text: t.text.trim().to_string(),
                start_s: t.start as f64,
                end_s: t.end as f64,
                confidence: None,
            });
        Ok(attach_punctuation(words))
    }
}

/// Parakeet emits punctuation as tokens of its own; attach each run of them to the word before.
pub fn attach_punctuation(words: impl IntoIterator<Item = TimedWord>) -> Vec<TimedWord> {
    let mut out: Vec<TimedWord> = Vec::new();
    for word in words {
        let punctuation = !word.text.chars().any(char::is_alphanumeric);
        match out.last_mut() {
            Some(last) if punctuation => last.text.push_str(&word.text),
            _ => out.push(word),
        }
    }
    out
}

#[cfg(test)]
#[path = "tests/parakeet_tdt.rs"]
mod tests;
