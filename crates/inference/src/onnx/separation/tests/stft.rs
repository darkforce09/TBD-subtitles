use super::*;

fn chirp(len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let t = i as f32 / 44_100.0;
            (2.0 * std::f32::consts::PI * (200.0 + 900.0 * t) * t).sin() * 0.5
        })
        .collect()
}

#[test]
fn inverse_undoes_forward() {
    for (n_fft, hop) in [(2048, 441), (7680, 1024), (64, 16)] {
        let stft = Stft::new(n_fft, hop);
        let signal = chirp(n_fft * 6 + 123);
        let spectrum = stft.forward(&signal);
        assert_eq!(spectrum.len(), stft.frames(signal.len()) * stft.bins());
        let back = stft.inverse(&spectrum, signal.len());
        let worst = signal
            .iter()
            .zip(&back)
            .map(|(a, b)| (a - b).abs())
            .fold(0f32, f32::max);
        assert!(worst < 1e-4, "n_fft {n_fft}: worst error {worst}");
    }
}

#[test]
fn a_pure_tone_lands_in_its_bin() {
    let stft = Stft::new(2048, 512);
    // Bin 64 of a 2048-point FFT at 44.1 kHz.
    let freq = 64.0 * 44_100.0 / 2048.0;
    let signal: Vec<f32> = (0..8192)
        .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / 44_100.0).sin())
        .collect();
    let spectrum = stft.forward(&signal);
    let frame = &spectrum[4 * stft.bins()..5 * stft.bins()];
    let loudest = (0..frame.len())
        .max_by(|&a, &b| frame[a].norm().total_cmp(&frame[b].norm()))
        .unwrap();
    assert_eq!(loudest, 64);
}

#[test]
fn reflection_mirrors_without_repeating_the_edge() {
    let signal = [1.0, 2.0, 3.0, 4.0];
    let got: Vec<f32> = (-3..7).map(|i| reflect(&signal, i)).collect();
    assert_eq!(got, vec![4.0, 3.0, 2.0, 1.0, 2.0, 3.0, 4.0, 3.0, 2.0, 1.0]);
}
