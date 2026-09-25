//! Sound events on both stems: window scores from a tagger, turned into candidate sound cues and
//! the music and singing stretches.
//!
//! **Role:** slide a fixed window along a 16 kHz stem, score each window with an AudioSet tagger,
//! smooth each class's scores over time, and cut stretches where a class stays over its
//! threshold for long enough.
//!
//! **Position:** called inside the sound-event worker (and by the stack spike tool); the tagger is
//! any [`Tagger`], such as `inference::onnx::ced::Ced`; `classes.rs` names the classes the
//! subtitles care about.
//!
//! **Signals and state:** one batch of windows at a time; one score row per window.
//!
//! **Invariants:** window `i` covers `[i × hop, i × hop + window)`; an event's times are the
//! centres of its first and last windows widened by half a hop.

pub mod classes;

use std::path::Path;

use job_model::outputs::SoundEvent;
use media_io::MediaError;
use media_io::pcm_stream::read_f32_range;

/// Scores audio windows: one probability per class per window.
pub trait Tagger {
    fn scores(&mut self, windows: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, String>;
}

impl Tagger for inference::onnx::ced::Ced {
    fn scores(&mut self, windows: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, String> {
        inference::onnx::ced::Ced::scores(self, windows).map_err(|e| e.to_string())
    }
}

/// How the stem is windowed.
#[derive(Debug, Clone, Copy)]
pub struct Windowing {
    pub window_s: f64,
    pub hop_s: f64,
    pub batch: usize,
}

impl Default for Windowing {
    /// 2 s windows every 0.5 s, 64 per model call.
    fn default() -> Windowing {
        Windowing {
            window_s: 2.0,
            hop_s: 0.5,
            batch: 64,
        }
    }
}

/// Every window's scores, in order.
pub fn score_stem(
    tagger: &mut dyn Tagger,
    stem: &Path,
    duration_s: f64,
    windowing: &Windowing,
) -> Result<Vec<Vec<f32>>, String> {
    let rate = 16_000.0;
    let window = (windowing.window_s * rate) as usize;
    let count = ((duration_s - windowing.window_s) / windowing.hop_s)
        .floor()
        .max(0.0) as usize
        + 1;
    let mut rows = Vec::with_capacity(count);
    let mut batch = Vec::with_capacity(windowing.batch);
    for i in 0..count {
        let start = (i as f64 * windowing.hop_s * rate) as u64;
        let mut samples =
            read_f32_range(stem, start, window).map_err(|e: MediaError| e.to_string())?;
        samples.resize(window, 0.0);
        batch.push(samples);
        if batch.len() == windowing.batch || i + 1 == count {
            rows.extend(tagger.scores(&batch)?);
            batch.clear();
        }
    }
    Ok(rows)
}

/// The rule that turns one class's scores into events.
#[derive(Debug, Clone, Copy)]
pub struct ClassRule {
    pub label: &'static str,
    pub index: usize,
    pub threshold: f32,
    pub min_s: f64,
}

/// Events for `rule` from window `rows`: scores smoothed by a three-window median, over the
/// threshold for at least `min_s`.
pub fn events(
    rows: &[Vec<f32>],
    rule: &ClassRule,
    windowing: &Windowing,
    stem: &str,
) -> Vec<SoundEvent> {
    let raw: Vec<f32> = rows
        .iter()
        .map(|r| r.get(rule.index).copied().unwrap_or(0.0))
        .collect();
    let smooth: Vec<f32> = (0..raw.len())
        .map(|i| {
            let mut three = [
                raw[i.saturating_sub(1)],
                raw[i],
                raw[(i + 1).min(raw.len() - 1)],
            ];
            three.sort_by(f32::total_cmp);
            three[1]
        })
        .collect();
    let centre = |i: usize| i as f64 * windowing.hop_s + windowing.window_s / 2.0;
    let mut out = Vec::new();
    let mut i = 0;
    while i < smooth.len() {
        if smooth[i] < rule.threshold {
            i += 1;
            continue;
        }
        let start = i;
        let mut peak = smooth[i];
        while i < smooth.len() && smooth[i] >= rule.threshold {
            peak = peak.max(smooth[i]);
            i += 1;
        }
        let (start_s, end_s) = (
            (centre(start) - windowing.hop_s / 2.0).max(0.0),
            centre(i - 1) + windowing.hop_s / 2.0,
        );
        if end_s - start_s >= rule.min_s {
            out.push(SoundEvent {
                label: rule.label.to_string(),
                stem: stem.to_string(),
                start_s,
                end_s,
                peak,
            });
        }
    }
    out
}

/// The mean score of class `index` over the windows whose centres lie in `[from_s, to_s)`.
pub fn mean_score(
    rows: &[Vec<f32>],
    index: usize,
    windowing: &Windowing,
    from_s: f64,
    to_s: f64,
) -> f32 {
    let picked: Vec<f32> = rows
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let c = *i as f64 * windowing.hop_s + windowing.window_s / 2.0;
            c >= from_s && c < to_s
        })
        .map(|(_, r)| r.get(index).copied().unwrap_or(0.0))
        .collect();
    if picked.is_empty() {
        0.0
    } else {
        picked.iter().sum::<f32>() / picked.len() as f32
    }
}

#[cfg(test)]
#[path = "tests/sound_events.rs"]
mod tests;
