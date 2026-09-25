use super::*;

/// Returns half of every window, so the separated vocals must be exactly half the mix.
struct Halver {
    layout: Layout,
    batch: usize,
    calls: usize,
}

impl WindowModel for Halver {
    fn layout(&self) -> Layout {
        self.layout.clone()
    }
    fn batch(&self) -> usize {
        self.batch
    }
    fn separate(&mut self, windows: &[Vec<f32>]) -> Result<Vec<Vec<f32>>, OnnxError> {
        self.calls += 1;
        assert!(
            windows
                .iter()
                .all(|w| w.len() == self.layout.window * CHANNELS)
        );
        Ok(windows
            .iter()
            .map(|w| w.iter().map(|s| s * 0.5).collect())
            .collect())
    }
}

fn trimmed(window: usize, trim: usize) -> Layout {
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

fn hamming(window: usize, step: usize) -> Layout {
    Layout {
        window,
        step,
        lead: 0,
        weights: (0..window)
            .map(|i| {
                let x = 2.0 * std::f32::consts::PI * i as f32 / (window - 1) as f32;
                0.54 - 0.46 * x.cos()
            })
            .collect(),
    }
}

fn run(layout: Layout, batch: usize, frames: usize, push_frames: usize) -> (Separated, Vec<f32>) {
    let track: Vec<f32> = (0..frames * CHANNELS)
        .map(|i| ((i * 7919) % 1000) as f32 / 1000.0 - 0.5)
        .collect();
    let mut driver = OverlapAdd::new(Halver {
        layout,
        batch,
        calls: 0,
    });
    let mut all = Separated::default();
    for piece in track.chunks(push_frames * CHANNELS) {
        let out = driver.push(piece).unwrap();
        all.mix.extend(out.mix);
        all.vocals.extend(out.vocals);
    }
    let (out, _) = driver.finish().unwrap();
    all.mix.extend(out.mix);
    all.vocals.extend(out.vocals);
    (all, track)
}

fn check(layout: Layout, batch: usize, frames: usize, push_frames: usize) {
    let (out, track) = run(layout, batch, frames, push_frames);
    assert_eq!(out.mix, track, "every mix sample once, in order");
    assert_eq!(out.vocals.len(), track.len());
    for (v, m) in out.vocals.iter().zip(&track) {
        assert!((v - m * 0.5).abs() < 1e-5, "vocals {v} for mix {m}");
    }
}

#[test]
fn trimmed_windows_cover_every_frame_once() {
    check(trimmed(100, 10), 1, 1234, 37);
    check(trimmed(100, 10), 4, 1234, 500);
    check(trimmed(100, 10), 3, 50, 7);
}

#[test]
fn hamming_windows_normalise_their_overlap() {
    check(hamming(120, 80), 1, 1000, 33);
    check(hamming(120, 80), 1, 90, 1000);
}

#[test]
fn an_empty_track_gives_nothing() {
    let (out, _) = run(trimmed(100, 10), 2, 0, 10);
    assert!(out.mix.is_empty() && out.vocals.is_empty());
}
