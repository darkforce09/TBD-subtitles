//! The NeMo log-mel frontend Parakeet was trained on: pre-emphasis 0.97, a 512-point STFT over
//! 400-sample Hann windows every 160 samples, 80 Slaney mel bands, a log with a 2^-24 guard, and
//! per-band normalisation. It follows parakeet-rs's frontend (MIT or Apache-2.0), which the crate
//! keeps private.
//!
//! **Role:** turn 16 kHz mono samples into the `[frames, 80]` features the CTC model reads.
//!
//! **Position:** used by `mod.rs`; computes with `realfft`.
//!
//! **Signals and state:** the FFT plan, window and mel filterbank, built once.
//!
//! **Invariants:** `floor(samples / 160)` frames, as NeMo counts them; every band has zero mean
//! and unit spread over the chunk.

use std::f64::consts::PI;
use std::sync::Arc;

use realfft::{RealFftPlanner, RealToComplex};

const N_FFT: usize = 512;
const WIN: usize = 400;
const HOP: usize = 160;
/// Mel bands.
pub const BANDS: usize = 80;
const RATE: f64 = 16_000.0;

/// The frontend's fixed parts.
pub struct Frontend {
    plan: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    /// `[band][bin]`.
    mel: Vec<Vec<f32>>,
}

impl Default for Frontend {
    fn default() -> Frontend {
        Frontend::new()
    }
}

impl Frontend {
    pub fn new() -> Frontend {
        let plan = RealFftPlanner::<f32>::new().plan_fft_forward(N_FFT);
        let window = (0..WIN)
            .map(|i| (0.5 - 0.5 * (2.0 * PI * i as f64 / (WIN as f64 - 1.0)).cos()) as f32)
            .collect();
        Frontend {
            plan,
            window,
            mel: slaney_filterbank(),
        }
    }

    /// Features `[frame][band]`, flattened.
    pub fn features(&self, samples: &[f32]) -> (usize, Vec<f32>) {
        let mut audio = Vec::with_capacity(samples.len());
        for (i, s) in samples.iter().enumerate() {
            audio.push(if i == 0 {
                *s
            } else {
                s - 0.97 * samples[i - 1]
            });
        }
        let pad = N_FFT / 2;
        let mut padded = vec![0f32; pad];
        padded.extend_from_slice(&audio);
        padded.resize(padded.len() + pad, 0.0);
        let frames = audio.len() / HOP;
        let bins = N_FFT / 2 + 1;
        let offset = (N_FFT - WIN) / 2;
        let mut input = self.plan.make_input_vec();
        let mut output = self.plan.make_output_vec();
        let mut scratch = self.plan.make_scratch_vec();
        let mut out = vec![0f32; frames * BANDS];
        let mut power = vec![0f32; bins];
        for frame in 0..frames {
            input.fill(0.0);
            let start = frame * HOP;
            for i in 0..WIN {
                input[offset + i] = padded[start + offset + i] * self.window[i];
            }
            let _ = self
                .plan
                .process_with_scratch(&mut input, &mut output, &mut scratch);
            for (k, p) in power.iter_mut().enumerate() {
                *p = output[k].norm_sqr();
            }
            for (band, weights) in self.mel.iter().enumerate() {
                let energy: f32 = weights.iter().zip(&power).map(|(w, p)| w * p).sum();
                out[frame * BANDS + band] = (energy + 2f32.powi(-24)).ln();
            }
        }
        if frames > 1 {
            for band in 0..BANDS {
                let mean = (0..frames).map(|f| out[f * BANDS + band]).sum::<f32>() / frames as f32;
                let var = (0..frames)
                    .map(|f| (out[f * BANDS + band] - mean).powi(2))
                    .sum::<f32>()
                    / (frames as f32 - 1.0);
                let std = var.sqrt() + 1e-5;
                for f in 0..frames {
                    out[f * BANDS + band] = (out[f * BANDS + band] - mean) / std;
                }
            }
        }
        (frames, out)
    }
}

fn hz_to_mel(hz: f64) -> f64 {
    const F_SP: f64 = 200.0 / 3.0;
    if hz < 1000.0 {
        hz / F_SP
    } else {
        1000.0 / F_SP + (hz / 1000.0).ln() / 0.068_751_777_420_949_12
    }
}

fn mel_to_hz(mel: f64) -> f64 {
    const F_SP: f64 = 200.0 / 3.0;
    let min_log_mel = 1000.0 / F_SP;
    if mel < min_log_mel {
        mel * F_SP
    } else {
        1000.0 * ((mel - min_log_mel) * 0.068_751_777_420_949_12).exp()
    }
}

/// librosa's Slaney-normalised mel filterbank, `[band][bin]`.
fn slaney_filterbank() -> Vec<Vec<f32>> {
    let bins = N_FFT / 2 + 1;
    let (low, high) = (hz_to_mel(0.0), hz_to_mel(RATE / 2.0));
    let points: Vec<f64> = (0..=BANDS + 1)
        .map(|i| mel_to_hz(low + (high - low) * i as f64 / (BANDS + 1) as f64))
        .collect();
    (0..BANDS)
        .map(|band| {
            let norm = 2.0 / (points[band + 2] - points[band]);
            (0..bins)
                .map(|k| {
                    let freq = k as f64 * RATE / N_FFT as f64;
                    let rise = (freq - points[band]) / (points[band + 1] - points[band]);
                    let fall = (points[band + 2] - freq) / (points[band + 2] - points[band + 1]);
                    (rise.min(fall).max(0.0) * norm) as f32
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/features.rs"]
mod tests;
