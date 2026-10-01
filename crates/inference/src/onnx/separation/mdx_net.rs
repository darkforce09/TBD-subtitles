//! UVR MDX-Net vocal models (Voc_FT, Kim_Vocal_2): the spectrum in, the vocal spectrum out.
//!
//! **Role:** run an MDX-Net ONNX model over stereo windows. Each window of `hop × (dim_t − 1)`
//! frames is transformed with a 7680-point STFT, cut to the model's `dim_f` bins, packed as
//! `[batch, 4, dim_f, dim_t]` (left real, left imaginary, right real, right imaginary), and the
//! model's output spectrum is turned back into samples and scaled by the model's compensation.
//!
//! **Position:** a [`WindowModel`] for `overlap_add.rs`; opened by the separation step's task.
//!
//! **Signals and state:** one ONNX session and one STFT; counts model seconds for the report.
//!
//! **Invariants:** the input shape is read from the model and checked before any audio is sent;
//! the window's `n_fft / 2` edge frames are weighted zero, as UVR trims them.

use std::path::Path;
use std::time::Instant;

use ort::session::Session;
use ort::value::Tensor;
use realfft::num_complex::Complex32;

use super::overlap_add::{CHANNELS, Layout, WindowModel};
use super::stft::Stft;
use crate::onnx::{Device, OnnxError, session};

/// Voc_FT's settings from UVR's model data: 7680-point FFT, hop 1024, 3072 bins, 256 frames.
pub const VOC_FT: MdxSettings = MdxSettings {
    n_fft: 7680,
    hop: 1024,
    dim_f: 3072,
    dim_t: 256,
    compensate: 1.021,
};

/// The STFT and packing settings of one MDX-Net model.
#[derive(Debug, Clone, Copy)]
pub struct MdxSettings {
    pub n_fft: usize,
    pub hop: usize,
    pub dim_f: usize,
    pub dim_t: usize,
    pub compensate: f32,
}

/// An opened MDX-Net model.
pub struct MdxNet {
    session: Session,
    settings: MdxSettings,
    stft: Stft,
    batch: usize,
    /// Seconds spent inside the model, for the report.
    pub model_s: f64,
    /// Seconds spent in the STFTs, for the report.
    pub stft_s: f64,
}

impl MdxNet {
    pub fn open(model: &Path, settings: MdxSettings, batch: usize) -> Result<MdxNet, OnnxError> {
        let session = session::open(model, Device::Cuda)?;
        let expected = format!("[-1, 4, {}, {}]", settings.dim_f, settings.dim_t);
        let described = session::describe(&session).join("; ");
        if !described.contains(&expected) {
            return Err(OnnxError::new(
                format!("checking {}", model.display()),
                format!("expected input shape {expected}, found {described}"),
            ));
        }
        Ok(MdxNet {
            session,
            stft: Stft::new(settings.n_fft, settings.hop),
            settings,
            batch: batch.max(1),
            model_s: 0.0,
            stft_s: 0.0,
        })
    }

    fn window(&self) -> usize {
        self.settings.hop * (self.settings.dim_t - 1)
    }

    /// Pack one window's two channels into the model's `[4, dim_f, dim_t]` layout.
    fn pack(&self, window: &[f32], out: &mut Vec<f32>) {
        let (dim_f, dim_t) = (self.settings.dim_f, self.settings.dim_t);
        let bins = self.stft.bins();
        for c in 0..CHANNELS {
            let channel: Vec<f32> = window.iter().skip(c).step_by(CHANNELS).copied().collect();
            let spectrum = self.stft.forward(&channel);
            for part in 0..2 {
                for f in 0..dim_f {
                    for t in 0..dim_t {
                        let z = spectrum[t * bins + f];
                        out.push(if part == 0 { z.re } else { z.im });
                    }
                }
            }
        }
    }

    /// Turn one `[4, dim_f, dim_t]` block of model output into interleaved stereo.
    fn unpack(&self, block: &[f32]) -> Vec<f32> {
        let (dim_f, dim_t) = (self.settings.dim_f, self.settings.dim_t);
        let bins = self.stft.bins();
        let len = self.window();
        let mut stereo = vec![0f32; len * CHANNELS];
        for c in 0..CHANNELS {
            let re = &block[(c * 2) * dim_f * dim_t..(c * 2 + 1) * dim_f * dim_t];
            let im = &block[(c * 2 + 1) * dim_f * dim_t..(c * 2 + 2) * dim_f * dim_t];
            let mut spectrum = vec![Complex32::new(0.0, 0.0); dim_t * bins];
            for f in 0..dim_f {
                for t in 0..dim_t {
                    spectrum[t * bins + f] = Complex32::new(re[f * dim_t + t], im[f * dim_t + t]);
                }
            }
            let samples = self.stft.inverse(&spectrum, len);
            for (i, s) in samples.iter().enumerate() {
                stereo[i * CHANNELS + c] = s * self.settings.compensate;
            }
        }
        stereo
    }
}

impl WindowModel for MdxNet {
    fn layout(&self) -> Layout {
        let window = self.window();
        let trim = self.settings.n_fft / 2;
        Layout {
            window,
            step: window - 2 * trim,
            lead: trim,
            weights: (0..window)
                .map(|i| {
                    if i >= trim && i < window - trim {
                        1.0
                    } else {
                        0.0
                    }
                })
                .collect(),
        }
    }

    fn batch(&self) -> usize {
        self.batch
    }

    fn separate(&mut self, windows: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, OnnxError> {
        let (dim_f, dim_t) = (self.settings.dim_f, self.settings.dim_t);
        let block = 4 * dim_f * dim_t;
        let started = Instant::now();
        let mut input = Vec::with_capacity(windows.len() * block);
        for window in windows {
            self.pack(window, &mut input);
        }
        self.stft_s += started.elapsed().as_secs_f64();
        let run = Instant::now();
        let tensor = Tensor::from_array(([windows.len(), 4, dim_f, dim_t], input))
            .map_err(|e| OnnxError::new("building the MDX-Net input", e))?;
        let outputs = self
            .session
            .run(ort::inputs![tensor])
            .map_err(|e| OnnxError::new("running MDX-Net", e))?;
        let (_, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| OnnxError::new("reading the MDX-Net output", e))?;
        let data = data.to_vec();
        drop(outputs);
        self.model_s += run.elapsed().as_secs_f64();
        let back = Instant::now();
        let vocals = (0..windows.len())
            .map(|b| self.unpack(&data[b * block..(b + 1) * block]))
            .collect();
        self.stft_s += back.elapsed().as_secs_f64();
        Ok(vocals)
    }
}
