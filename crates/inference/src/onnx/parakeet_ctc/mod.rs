//! NVIDIA Parakeet-CTC-0.6B: frame-level token log-probabilities for forced alignment.
//!
//! **Role:** turn a 16 kHz mono block into CTC log-probabilities, one row per 80 ms encoder
//! frame over the model's 1025 tokens, and split text into the model's BPE tokens, so a Viterbi
//! pass can place every token of a known transcript in time.
//!
//! **Position:** used by the alignment step's task, which hands the grid to the alignment stage,
//! and by the stack spike tool; runs the ONNX graph through
//! `session.rs` on CUDA and tokenises with the model's own `tokenizer.json`.
//!
//! **Signals and state:** one ONNX session, the log-mel frontend and the tokenizer.
//!
//! **Invariants:** token 1024 (`<pad>`) is the CTC blank; each row of the grid sums to one in
//! probability (a log-softmax is applied here).

pub mod features;

use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

use crate::onnx::{Device, OnnxError, session};
use features::{BANDS, Frontend};

/// The model id and folder name.
pub const MODEL: &str = "parakeet-ctc-0.6b";
/// The blank token.
pub const BLANK: usize = 1024;
/// Seconds per encoder frame: a 10 ms hop, subsampled eight times.
pub const FRAME_S: f64 = 0.08;

/// Log-probabilities `[frame][token]`, flattened.
#[derive(Debug, Clone)]
pub struct CtcGrid {
    pub frames: usize,
    pub vocab: usize,
    pub log_probs: Vec<f32>,
}

impl CtcGrid {
    pub fn row(&self, frame: usize) -> &[f32] {
        &self.log_probs[frame * self.vocab..(frame + 1) * self.vocab]
    }
}

/// An opened Parakeet-CTC model.
pub struct ParakeetCtc {
    session: Session,
    frontend: Frontend,
    tokenizer: Tokenizer,
}

impl ParakeetCtc {
    /// Open the model folder (`model.onnx` with its data, `tokenizer.json`).
    pub fn open(dir: &Path, device: Device) -> Result<ParakeetCtc, OnnxError> {
        let session = session::open(&dir.join("model.onnx"), device)?;
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer.json"))
            .map_err(|e| OnnxError::new("reading the Parakeet-CTC tokenizer", e))?;
        Ok(ParakeetCtc {
            session,
            frontend: Frontend::new(),
            tokenizer,
        })
    }

    /// The BPE tokens of one spoken word, as the model spells it.
    pub fn tokens(&self, word: &str) -> Result<Vec<usize>, OnnxError> {
        let encoding = self
            .tokenizer
            .encode(word, false)
            .map_err(|e| OnnxError::new(format!("tokenising {word:?}"), e))?;
        Ok(encoding.get_ids().iter().map(|&id| id as usize).collect())
    }

    /// The grid's best-path text: argmax per frame, repeats collapsed, blanks removed.
    pub fn greedy_text(&self, grid: &CtcGrid) -> Result<String, OnnxError> {
        let mut ids = Vec::new();
        let mut last = BLANK;
        for frame in 0..grid.frames {
            let row = grid.row(frame);
            let best = (0..row.len())
                .max_by(|&a, &b| row[a].total_cmp(&row[b]))
                .unwrap_or(BLANK);
            if best != BLANK && best != last {
                ids.push(best as u32);
            }
            last = best;
        }
        self.tokenizer
            .decode(&ids, true)
            .map_err(|e| OnnxError::new("decoding CTC tokens", e))
    }

    /// The CTC grid of one 16 kHz mono block.
    pub fn log_probs(&mut self, samples: &[f32]) -> Result<CtcGrid, OnnxError> {
        let (frames, features) = self.frontend.features(samples);
        if frames == 0 {
            return Ok(CtcGrid {
                frames: 0,
                vocab: BLANK + 1,
                log_probs: Vec::new(),
            });
        }
        let input = Tensor::from_array(([1, frames, BANDS], features))
            .map_err(|e| OnnxError::new("building the CTC input", e))?;
        let mask = Tensor::from_array(([1, frames], vec![1i64; frames]))
            .map_err(|e| OnnxError::new("building the CTC mask", e))?;
        let outputs = self
            .session
            .run(ort::inputs!["input_features" => input, "attention_mask" => mask])
            .map_err(|e| OnnxError::new("running Parakeet-CTC", e))?;
        let (shape, data) = outputs["logits"]
            .try_extract_tensor::<f32>()
            .map_err(|e| OnnxError::new("reading the CTC logits", e))?;
        let (rows, vocab) = (shape[1] as usize, shape[2] as usize);
        let mut log_probs = data.to_vec();
        for row in log_probs.chunks_mut(vocab) {
            let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let sum: f32 = row.iter().map(|x| (x - max).exp()).sum();
            let log_sum = max + sum.ln();
            for x in row.iter_mut() {
                *x -= log_sum;
            }
        }
        Ok(CtcGrid {
            frames: rows,
            vocab,
            log_probs,
        })
    }
}
