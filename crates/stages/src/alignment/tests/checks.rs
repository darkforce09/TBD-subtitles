use super::*;

#[test]
fn the_median_and_share_come_from_the_start_differences() {
    let aligned = [Some((1.0, 1.2)), Some((2.05, 2.3)), None, Some((4.5, 4.8))];
    let reference = [(1.0, 1.2), (2.0, 2.3), (3.0, 3.1), (4.0, 4.8)];
    let c = compare(&aligned, &reference);
    assert_eq!(c.words, 3);
    assert!((c.median_start_diff_s - 0.05).abs() < 1e-9);
    assert!((c.share_over_200ms - 1.0 / 3.0).abs() < 1e-9);
}

#[test]
fn evenly_spread_words_are_flagged_and_real_speech_is_not() {
    let spread = [(0.0, 0.2), (0.3, 0.5), (0.6, 0.8), (0.9, 1.1)];
    assert_eq!(suspicious_runs(&spread), 1);
    let speech = [(0.0, 0.31), (0.35, 0.52), (0.6, 0.97), (1.1, 1.2)];
    assert_eq!(suspicious_runs(&speech), 0);
    let flat = [(1.0, 1.0), (1.0, 1.0), (1.0, 1.0)];
    assert_eq!(suspicious_runs(&flat), 1);
}
