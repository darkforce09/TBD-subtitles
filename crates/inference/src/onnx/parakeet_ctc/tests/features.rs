use super::*;

#[test]
fn frames_follow_nemo_and_bands_are_normalised() {
    let samples: Vec<f32> = (0..16_000)
        .map(|i| ((i as f32) * 0.07).sin() * 0.3 + ((i as f32) * 0.011).sin() * 0.1)
        .collect();
    let (frames, features) = Frontend::new().features(&samples);
    assert_eq!(frames, 100);
    assert_eq!(features.len(), 100 * BANDS);
    for band in [0, 40, 79] {
        let mean: f32 = (0..frames).map(|f| features[f * BANDS + band]).sum::<f32>() / 100.0;
        assert!(mean.abs() < 1e-3, "band {band} mean {mean}");
    }
}

#[test]
fn the_filterbank_covers_the_spectrum() {
    let bank = slaney_filterbank();
    assert_eq!(bank.len(), BANDS);
    assert!(bank.iter().all(|band| band.iter().any(|w| *w > 0.0)));
}
