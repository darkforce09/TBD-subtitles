use super::*;

fn tone(freq: f64, seconds: f64) -> Vec<f32> {
    let n = (44_100.0 * seconds) as usize;
    (0..n)
        .map(|i| (2.0 * std::f64::consts::PI * freq * i as f64 / 44_100.0).sin() as f32 * 0.5)
        .collect()
}

fn resample(input: &[f32], piece: usize) -> Vec<f32> {
    let mut r = Resampler::new();
    let mut out = Vec::new();
    for chunk in input.chunks(piece) {
        out.extend(r.push(chunk));
    }
    out.extend(r.finish());
    out
}

fn rms(signal: &[f32]) -> f32 {
    (signal.iter().map(|s| s * s).sum::<f32>() / signal.len() as f32).sqrt()
}

#[test]
fn one_second_in_gives_sixteen_thousand_out() {
    assert_eq!(resample(&tone(440.0, 1.0), 1000).len(), 16_000);
}

#[test]
fn a_passband_tone_keeps_its_level_and_phase() {
    let input = tone(1000.0, 1.0);
    let out = resample(&input, 777);
    let middle = &out[2000..14_000];
    assert!(
        (rms(middle) - 0.5 / 2f32.sqrt()).abs() < 0.01,
        "rms {}",
        rms(middle)
    );
    for (n, got) in middle.iter().enumerate().step_by(97) {
        let t = (n + 2000) as f64 / 16_000.0;
        let expected = ((2.0 * std::f64::consts::PI * 1000.0 * t).sin() * 0.5) as f32;
        assert!(
            (got - expected).abs() < 0.01,
            "sample {n}: {got} vs {expected}"
        );
    }
}

#[test]
fn a_tone_above_eight_kilohertz_is_removed() {
    let out = resample(&tone(12_000.0, 1.0), 4096);
    assert!(
        rms(&out[2000..14_000]) < 0.005,
        "rms {}",
        rms(&out[2000..14_000])
    );
}

#[test]
fn piece_sizes_do_not_change_the_output() {
    let input = tone(3000.0, 0.5);
    assert_eq!(resample(&input, 1), resample(&input, input.len()));
}
