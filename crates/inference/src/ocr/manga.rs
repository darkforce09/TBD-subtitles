//! Character decoding for difficult Japanese image crops.
//!
//! **Role:** run the pinned manga-ocr ViT encoder and character decoder.
//! **Position:** independent local OCR fallback behind the PP-OCRv5 reader.
//! **Signals and state:** two CUDA sessions, a character vocabulary and four bounded beams.
//! **Invariants:** inputs are grayscale RGB at 224 square; decoding stops at EOS or 300 tokens;
//! scores are geometric mean token probabilities and are not calibrated correctness estimates.

use std::path::Path;

use image::{RgbImage, imageops::FilterType};
use ort::session::Session;
use ort::value::Tensor;

use super::{OcrError, check_image, open_session, strict_cuda_environment};

const BEAMS: usize = 4;
const MAX_LENGTH: usize = 300;
const EOS: i64 = 3;
const START: i64 = 2;

pub struct MangaReader {
    encoder: Session,
    decoder: Session,
    vocabulary: Vec<String>,
}

#[derive(Clone)]
struct Beam {
    tokens: Vec<i64>,
    log_probability: f64,
}

impl Beam {
    fn rank(&self) -> f64 {
        self.log_probability / (self.tokens.len().saturating_sub(1).max(1) as f64).powi(2)
    }
}

impl MangaReader {
    pub fn open(models_root: &Path) -> Result<Self, OcrError> {
        strict_cuda_environment()?;
        let dir = models_root.join("manga-ocr");
        let vocabulary: Vec<String> = std::fs::read_to_string(dir.join("vocab.txt"))?
            .lines()
            .map(str::to_owned)
            .collect();
        if vocabulary.len() != 6144
            || vocabulary[START as usize] != "[CLS]"
            || vocabulary[EOS as usize] != "[SEP]"
        {
            return Err("Manga OCR vocabulary does not match the pinned character decoder".into());
        }
        Ok(Self {
            encoder: open_session(&dir.join("encoder.onnx"))?,
            decoder: open_session(&dir.join("decoder.onnx"))?,
            vocabulary,
        })
    }

    pub fn read(&mut self, image: &RgbImage) -> Result<(String, f64), OcrError> {
        check_image(image)?;
        let gray = image::imageops::grayscale(image);
        let resized = image::imageops::resize(&gray, 224, 224, FilterType::Triangle);
        let mut pixels = Vec::with_capacity(3 * 224 * 224);
        for _ in 0..3 {
            pixels.extend(
                resized
                    .pixels()
                    .map(|p| (f32::from(p[0]) / 255.0 - 0.5) / 0.5),
            );
        }
        let input = Tensor::from_array(([1, 3, 224, 224], pixels))?;
        let (hidden_shape, hidden) = {
            let output = self.encoder.run(ort::inputs!["pixel_values" => input])?;
            let (shape, values) = output["last_hidden_state"].try_extract_tensor::<f32>()?;
            if shape.len() != 3 || shape[0] != 1 || shape[1] != 197 || shape[2] != 768 {
                return Err("Manga OCR encoder returned an unexpected shape".into());
            }
            ([1usize, 197, 768], values.to_vec())
        };
        let hidden = Tensor::from_array((hidden_shape, hidden))?;
        let beam = decode(&mut self.decoder, &hidden, self.vocabulary.len())?;
        let complete = beam.tokens.last() == Some(&EOS);
        let text = postprocess(&beam.tokens, &self.vocabulary);
        let count = beam.tokens.len().saturating_sub(1).max(1) as f64;
        let mut confidence = (beam.log_probability / count).exp().clamp(0.0, 1.0);
        if !complete || text.is_empty() || beam.tokens.contains(&1) {
            confidence = confidence.min(0.49);
        }
        tracing::debug!(
            model = "manga-ocr",
            text,
            confidence,
            complete,
            "OCR second reading"
        );
        Ok((text, confidence))
    }
}

fn decode(decoder: &mut Session, hidden: &Tensor<f32>, vocab: usize) -> Result<Beam, OcrError> {
    let mut active = vec![Beam {
        tokens: vec![START],
        log_probability: 0.0,
    }];
    let mut finished = Vec::<Beam>::new();
    while active[0].tokens.len() < MAX_LENGTH {
        let mut candidates = Vec::with_capacity(BEAMS * BEAMS * 2);
        for beam in &active {
            let ids = Tensor::from_array(([1, beam.tokens.len()], beam.tokens.clone()))?;
            let output = decoder.run(ort::inputs![
                "input_ids" => ids,
                "encoder_hidden_states" => hidden,
            ])?;
            let (shape, logits) = output["logits"].try_extract_tensor::<f32>()?;
            if shape.len() != 3
                || shape[0] != 1
                || shape[1] != beam.tokens.len() as i64
                || shape[2] != vocab as i64
                || logits.len() != beam.tokens.len() * vocab
            {
                return Err("Manga OCR decoder returned unexpected logits".into());
            }
            let row = &logits[logits.len() - vocab..];
            for (token, log_probability) in next_tokens(row, &beam.tokens)? {
                let mut next = beam.clone();
                next.tokens.push(token);
                next.log_probability += log_probability;
                candidates.push(next);
            }
        }
        candidates.sort_by(|a, b| b.log_probability.total_cmp(&a.log_probability));
        active.clear();
        for (rank, beam) in candidates.into_iter().enumerate() {
            if beam.tokens.last() == Some(&EOS) {
                if rank < BEAMS {
                    finished.push(beam);
                }
            } else if active.len() < BEAMS {
                active.push(beam);
            }
            if active.len() == BEAMS {
                break;
            }
        }
        finished.sort_by(|a, b| b.rank().total_cmp(&a.rank()));
        finished.truncate(BEAMS);
        // The pinned model's early_stopping=true stops once four completed beams exist.
        if finished.len() == BEAMS || active.is_empty() {
            break;
        }
    }
    if finished.len() < BEAMS {
        finished.extend(active);
    }
    finished
        .into_iter()
        .max_by(|a, b| a.rank().total_cmp(&b.rank()))
        .ok_or_else(|| "Manga OCR decoder produced no hypotheses".into())
}

fn next_tokens(logits: &[f32], prefix: &[i64]) -> Result<Vec<(i64, f64)>, OcrError> {
    if logits.iter().any(|v| !v.is_finite()) {
        return Err("Manga OCR decoder returned nonfinite logits".into());
    }
    let maximum = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let normalizer = f64::from(maximum)
        + logits
            .iter()
            .map(|v| f64::from(*v - maximum).exp())
            .sum::<f64>()
            .ln();
    let mut candidates: Vec<_> = logits
        .iter()
        .enumerate()
        .filter(|(token, _)| !repeated_trigram(prefix, *token as i64))
        .map(|(token, logit)| (token as i64, f64::from(*logit) - normalizer))
        .collect();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
    candidates.truncate(2 * BEAMS);
    Ok(candidates)
}

fn repeated_trigram(prefix: &[i64], next: i64) -> bool {
    if prefix.len() < 2 {
        return false;
    }
    let tail = &prefix[prefix.len() - 2..];
    prefix
        .windows(3)
        .any(|p| p[0] == tail[0] && p[1] == tail[1] && p[2] == next)
}

fn postprocess(tokens: &[i64], vocabulary: &[String]) -> String {
    let mut text = String::new();
    for &token in tokens {
        let Some(piece) = usize::try_from(token)
            .ok()
            .and_then(|id| vocabulary.get(id))
        else {
            continue;
        };
        if matches!(
            piece.as_str(),
            "[PAD]" | "[UNK]" | "[CLS]" | "[SEP]" | "[MASK]"
        ) {
            continue;
        }
        text.push_str(piece.strip_prefix("##").unwrap_or(piece));
    }
    let text: Vec<char> = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| {
            if c == '…' {
                vec!['.', '.', '.']
            } else {
                vec![c]
            }
        })
        .collect();
    let mut normalized = String::new();
    for (i, &c) in text.iter().enumerate() {
        let is_dot = |c: &char| matches!(c, '・' | '.');
        let repeated_dot = is_dot(&c)
            && (i
                .checked_sub(1)
                .and_then(|j| text.get(j))
                .is_some_and(is_dot)
                || text.get(i + 1).is_some_and(is_dot));
        let c = if repeated_dot { '.' } else { c };
        normalized.push(if c.is_ascii_graphic() {
            char::from_u32(u32::from(c) + 0xfee0).unwrap_or(c)
        } else {
            c
        });
    }
    normalized
}
