//! A streaming windowed-sinc resampler from 44.1 kHz to 16 kHz (the rational ratio 160/441).
//!
//! **Role:** turn the separated 44.1 kHz stems into the 16 kHz mono the speech models read,
//! without holding the track: samples go in as they come and come out as soon as the filter has
//! seen enough of them.
//!
//! **Position:** used by `mod.rs` for the vocal and background stems.
//!
//! **Signals and state:** the filter table (160 phases) and the last few input samples.
//!
//! **Invariants:** output sample `n` sits at input time `n × 441 / 160`; content below 7.2 kHz
//! passes, content above 8 kHz is removed before it can fold back.

const UP: usize = 160;
const DOWN: usize = 441;
/// Filter half-width in input samples.
const HALF: usize = 32;
/// Passband edge relative to the input rate's Nyquist frequency (7.2 kHz of 22.05 kHz).
const CUTOFF: f64 = 7_200.0 / 22_050.0;

/// A 44.1 kHz to 16 kHz mono resampler.
pub struct Resampler {
    /// `[phase][tap]`, taps for input offsets `-HALF + 1 ..= HALF`.
    table: Vec<[f32; 2 * HALF]>,
    input: Vec<f32>,
    /// The absolute index of `input[0]`.
    input_start: usize,
    pushed: usize,
    next_out: usize,
}

impl Default for Resampler {
    fn default() -> Resampler {
        Resampler::new()
    }
}

impl Resampler {
    pub fn new() -> Resampler {
        let table = (0..UP)
            .map(|phase| {
                let frac = phase as f64 / UP as f64;
                let mut taps = [0f32; 2 * HALF];
                for (j, tap) in taps.iter_mut().enumerate() {
                    // Distance from the output instant to input sample `base - HALF + 1 + j`.
                    let x = (j as f64 - (HALF as f64 - 1.0)) - frac;
                    *tap = (CUTOFF * sinc(CUTOFF * x) * blackman(x / HALF as f64)) as f32;
                }
                taps
            })
            .collect();
        Resampler {
            table,
            input: Vec::new(),
            input_start: 0,
            pushed: 0,
            next_out: 0,
        }
    }

    /// Add 44.1 kHz samples; returns the 16 kHz samples now complete.
    pub fn push(&mut self, samples: &[f32]) -> Vec<f32> {
        self.input.extend_from_slice(samples);
        self.pushed += samples.len();
        self.drain(false)
    }

    /// Flush the tail, as if the track were followed by silence.
    pub fn finish(mut self) -> Vec<f32> {
        self.drain(true)
    }

    fn drain(&mut self, last: bool) -> Vec<f32> {
        let total_out = (self.pushed * UP).div_ceil(DOWN);
        let mut out = Vec::new();
        loop {
            if last && self.next_out >= total_out {
                break;
            }
            let position = self.next_out * DOWN;
            let base = position / UP;
            let phase = position % UP;
            if !last && base + HALF >= self.pushed {
                break;
            }
            let taps = &self.table[phase];
            let mut acc = 0f32;
            for (j, tap) in taps.iter().enumerate() {
                let index = base as isize - HALF as isize + 1 + j as isize;
                if index >= 0 && (index as usize) < self.pushed {
                    acc += self.input[index as usize - self.input_start] * tap;
                }
            }
            out.push(acc);
            self.next_out += 1;
        }
        // Keep what the next output still needs.
        let next_base = (self.next_out * DOWN) / UP;
        let keep_from = next_base.saturating_sub(HALF).max(self.input_start);
        let drop = (keep_from - self.input_start).min(self.input.len());
        self.input.drain(..drop);
        self.input_start += drop;
        out
    }
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        (std::f64::consts::PI * x).sin() / (std::f64::consts::PI * x)
    }
}

/// A Blackman window over `-1..=1`.
fn blackman(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        return 0.0;
    }
    let t = std::f64::consts::PI * (x + 1.0);
    0.42 - 0.5 * t.cos() + 0.08 * (2.0 * t).cos()
}

#[cfg(test)]
#[path = "tests/resample.rs"]
mod tests;
