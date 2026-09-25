//! The short-time Fourier transform around the separation models, matching `torch.stft` and
//! `torch.istft` with `center=True`, reflect padding and a periodic Hann window.
//!
//! **Role:** turn one channel of samples into complex frames and back, so an ONNX model whose
//! graph has its STFT removed receives exactly the spectrum it was trained on.
//!
//! **Position:** used by `mdx_net.rs` and `mel_roformer.rs`; computes with `realfft`.
//!
//! **Signals and state:** the FFT plans and the window, built once per model.
//!
//! **Invariants:** `inverse(forward(x))` returns `x` (within float error) for any `x` longer than
//! half the FFT size; frames are laid out `[frame][bin]` with `n_fft / 2 + 1` bins.

use std::sync::Arc;

use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

/// A forward and inverse STFT of fixed size and hop.
pub struct Stft {
    n_fft: usize,
    hop: usize,
    window: Vec<f32>,
    forward: Arc<dyn RealToComplex<f32>>,
    inverse: Arc<dyn ComplexToReal<f32>>,
}

impl Stft {
    pub fn new(n_fft: usize, hop: usize) -> Stft {
        let mut planner = RealFftPlanner::<f32>::new();
        let window = (0..n_fft)
            .map(|i| {
                let phase = 2.0 * std::f64::consts::PI * i as f64 / n_fft as f64;
                (0.5 - 0.5 * phase.cos()) as f32
            })
            .collect();
        Stft {
            n_fft,
            hop,
            window,
            forward: planner.plan_fft_forward(n_fft),
            inverse: planner.plan_fft_inverse(n_fft),
        }
    }

    /// Bins per frame.
    pub fn bins(&self) -> usize {
        self.n_fft / 2 + 1
    }

    /// Frames for a signal of `len` samples.
    pub fn frames(&self, len: usize) -> usize {
        1 + len / self.hop
    }

    /// The spectrum of `signal`, `[frame][bin]`.
    pub fn forward(&self, signal: &[f32]) -> Vec<Complex32> {
        let half = self.n_fft / 2;
        let frames = self.frames(signal.len());
        let bins = self.bins();
        let mut out = vec![Complex32::new(0.0, 0.0); frames * bins];
        let mut input = self.forward.make_input_vec();
        let mut scratch = self.forward.make_scratch_vec();
        for frame in 0..frames {
            let start = (frame * self.hop) as isize - half as isize;
            for (i, slot) in input.iter_mut().enumerate() {
                *slot = reflect(signal, start + i as isize) * self.window[i];
            }
            let spectrum = &mut out[frame * bins..(frame + 1) * bins];
            // Sizes match the plan, so the transform cannot fail.
            let _ = self
                .forward
                .process_with_scratch(&mut input, spectrum, &mut scratch);
        }
        out
    }

    /// The signal of `len` samples whose spectrum is `spectrum` (`[frame][bin]`).
    pub fn inverse(&self, spectrum: &[Complex32], len: usize) -> Vec<f32> {
        let half = self.n_fft / 2;
        let bins = self.bins();
        let frames = spectrum.len() / bins;
        let total = self.n_fft + self.hop * frames.saturating_sub(1);
        let mut signal = vec![0f32; total];
        let mut envelope = vec![0f32; total];
        let mut buffer = self.inverse.make_input_vec();
        let mut output = self.inverse.make_output_vec();
        let mut scratch = self.inverse.make_scratch_vec();
        let scale = 1.0 / self.n_fft as f32;
        for frame in 0..frames {
            buffer.copy_from_slice(&spectrum[frame * bins..(frame + 1) * bins]);
            // A real signal's spectrum has real DC and Nyquist bins; realfft insists on it.
            buffer[0].im = 0.0;
            buffer[bins - 1].im = 0.0;
            let _ = self
                .inverse
                .process_with_scratch(&mut buffer, &mut output, &mut scratch);
            let start = frame * self.hop;
            for i in 0..self.n_fft {
                signal[start + i] += output[i] * scale * self.window[i];
                envelope[start + i] += self.window[i] * self.window[i];
            }
        }
        (0..len)
            .map(|i| {
                let at = i + half;
                if at < total && envelope[at] > 1e-8 {
                    signal[at] / envelope[at]
                } else {
                    0.0
                }
            })
            .collect()
    }
}

/// `signal[index]` with reflection at both ends, as `torch.stft` pads with `center=True`.
fn reflect(signal: &[f32], index: isize) -> f32 {
    let len = signal.len() as isize;
    if len == 0 {
        return 0.0;
    }
    if len == 1 {
        return signal[0];
    }
    let period = 2 * (len - 1);
    let mut i = index.rem_euclid(period);
    if i >= len {
        i = period - i;
    }
    signal[i as usize]
}

#[cfg(test)]
#[path = "tests/stft.rs"]
mod tests;
