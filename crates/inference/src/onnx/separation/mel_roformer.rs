//! The Mel-Band RoFormer vocal model with its STFT outside the graph: a complex mask per bin.
//!
//! **Role:** run the host-STFT Mel-Band RoFormer export over 11-second stereo windows. Each
//! window gets a 2048-point STFT (hop 441, 1101 frames), packed as `[1, 2050, 1101, 2]` with bin
//! index `2 × freq + channel`; the model's complex mask is multiplied onto the spectrum and the
//! result turned back into samples. Windows overlap with a Hamming weight and an 8-second step.
//!
//! **Position:** a [`WindowModel`] for `overlap_add.rs`; opened by the separation step's task.
//!
//! **Signals and state:** one ONNX session and one STFT; counts model seconds for the report.
//!
//! **Invariants:** the model's fixed input shape is checked before any audio is sent; one window
//! per call, as the export has a batch of one.

use std::path::Path;
use std::time::Instant;

use ort::session::Session;
use ort::value::Tensor;
use realfft::num_complex::Complex32;

use super::overlap_add::{CHANNELS, Layout, WindowModel};
use super::stft::Stft;
use crate::onnx::{Device, OnnxError, session};

const N_FFT: usize = 2048;
const HOP: usize = 441;
const FRAMES: usize = 1101;
/// Eight seconds at 44.1 kHz.
const STEP: usize = 352_800;

/// An opened Mel-Band RoFormer.
pub struct MelRoformer {
    session: Session,
    stft: Stft,
    pub model_s: f64,
    pub stft_s: f64,
}

impl MelRoformer {
    pub fn open(model: &Path) -> Result<MelRoformer, OnnxError> {
        let session = session::open(model, Device::Cuda)?;
        let bins = N_FFT / 2 + 1;
        let expected = format!("[1, {}, {FRAMES}, 2]", bins * CHANNELS);
        let described = session::describe(&session).join("; ");
        if !described.contains(&expected) {
            return Err(OnnxError::new(
                format!("checking {}", model.display()),
                format!("expected input shape {expected}, found {described}"),
            ));
        }
        Ok(MelRoformer {
            session,
            stft: Stft::new(N_FFT, HOP),
            model_s: 0.0,
            stft_s: 0.0,
        })
    }
}

impl WindowModel for MelRoformer {
    fn layout(&self) -> Layout {
        let window = HOP * (FRAMES - 1);
        Layout {
            window,
            step: STEP,
            lead: 0,
            weights: (0..window)
                .map(|i| {
                    let x = 2.0 * std::f64::consts::PI * i as f64 / (window - 1) as f64;
                    (0.54 - 0.46 * x.cos()) as f32
                })
                .collect(),
        }
    }

    fn batch(&self) -> usize {
        1
    }

    fn separate(&mut self, windows: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, OnnxError> {
        let bins = self.stft.bins();
        let len = HOP * (FRAMES - 1);
        let mut vocals = Vec::with_capacity(windows.len());
        for window in windows {
            let started = Instant::now();
            let spectra: Vec<Vec<Complex32>> = (0..CHANNELS)
                .map(|c| {
                    let channel: Vec<f32> =
                        window.iter().skip(c).step_by(CHANNELS).copied().collect();
                    self.stft.forward(&channel)
                })
                .collect();
            let mut input = vec![0f32; bins * CHANNELS * FRAMES * 2];
            for f in 0..bins {
                for (c, spectrum) in spectra.iter().enumerate() {
                    let row = (2 * f + c) * FRAMES * 2;
                    for t in 0..FRAMES {
                        let z = spectrum[t * bins + f];
                        input[row + t * 2] = z.re;
                        input[row + t * 2 + 1] = z.im;
                    }
                }
            }
            self.stft_s += started.elapsed().as_secs_f64();
            let run = Instant::now();
            let tensor = Tensor::from_array(([1, bins * CHANNELS, FRAMES, 2], input))
                .map_err(|e| OnnxError::new("building the RoFormer input", e))?;
            let outputs = self
                .session
                .run(ort::inputs![tensor])
                .map_err(|e| OnnxError::new("running the RoFormer", e))?;
            let (_, mask) = outputs[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| OnnxError::new("reading the RoFormer mask", e))?;
            let mask = mask.to_vec();
            drop(outputs);
            self.model_s += run.elapsed().as_secs_f64();
            let back = Instant::now();
            let mut stereo = vec![0f32; len * CHANNELS];
            for (c, spectrum) in spectra.iter().enumerate() {
                let mut masked = spectrum.clone();
                for f in 0..bins {
                    let row = (2 * f + c) * FRAMES * 2;
                    for t in 0..FRAMES {
                        let m = Complex32::new(mask[row + t * 2], mask[row + t * 2 + 1]);
                        masked[t * bins + f] = spectrum[t * bins + f] * m;
                    }
                }
                for (i, s) in self.stft.inverse(&masked, len).iter().enumerate() {
                    stereo[i * CHANNELS + c] = *s;
                }
            }
            self.stft_s += back.elapsed().as_secs_f64();
            vocals.push(stereo);
        }
        Ok(vocals)
    }
}
