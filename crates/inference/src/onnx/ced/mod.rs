//! CED, the consistent-ensemble-distilled AudioSet tagger: raw 16 kHz audio in, a probability for
//! each of the 527 AudioSet classes out.
//!
//! **Role:** score batches of equal-length audio windows; the graph takes the waveform and holds
//! its own feature extraction and its final sigmoid, so the output is used as it comes.
//!
//! **Position:** used by the sound-event stage and the stack spike tool; runs through
//! `session.rs` on CUDA.
//!
//! **Signals and state:** one ONNX session.
//!
//! **Invariants:** every window of a batch has the same length; each output row holds 527
//! independent probabilities (multi-label), not a distribution.

use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;

use crate::onnx::{Device, OnnxError, session};

/// The model id and folder name.
pub const MODEL: &str = "ced-base";
/// AudioSet's rated classes.
pub const CLASSES: usize = 527;

/// An opened CED model.
pub struct Ced {
    session: Session,
}

impl Ced {
    pub fn open(dir: &Path, device: Device) -> Result<Ced, OnnxError> {
        Ok(Ced {
            session: session::open(&dir.join("model.onnx"), device)?,
        })
    }

    /// One row of 527 probabilities per window; all windows must be the same length.
    pub fn scores(&mut self, windows: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, OnnxError> {
        let Some(first) = windows.first() else {
            return Ok(Vec::new());
        };
        let len = first.len();
        if windows.iter().any(|w| w.len() != len) {
            return Err(OnnxError::new(
                "scoring with CED",
                "windows differ in length",
            ));
        }
        let flat: Vec<f32> = windows.iter().flatten().copied().collect();
        let input = Tensor::from_array(([windows.len(), len], flat))
            .map_err(|e| OnnxError::new("building the CED input", e))?;
        let outputs = self
            .session
            .run(ort::inputs![input])
            .map_err(|e| OnnxError::new("running CED", e))?;
        // The export ends in its own sigmoid: the output named `logits` holds probabilities.
        let (_, probabilities) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| OnnxError::new("reading the CED output", e))?;
        Ok(probabilities.chunks(CLASSES).map(<[f32]>::to_vec).collect())
    }
}
