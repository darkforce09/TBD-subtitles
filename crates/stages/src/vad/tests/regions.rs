use super::*;

const F: f64 = 0.1;

#[test]
fn frames_over_the_threshold_become_padded_regions() {
    let scores = [0.0, 0.9, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.8, 0.0];
    let regions = from_scores(&scores, F, 0.5, 0.05, 0.0, 1.1);
    assert_eq!(regions.len(), 2);
    assert!((regions[0].start_s - 0.05).abs() < 1e-9);
    assert!((regions[0].end_s - 0.35).abs() < 1e-9);
    assert!((regions[1].start_s - 0.85).abs() < 1e-9);
}

#[test]
fn close_regions_merge_and_padding_stays_inside_the_track() {
    let scores = [0.9, 0.0, 0.0, 0.9];
    let regions = from_scores(&scores, F, 0.5, 0.2, 0.3, 0.4);
    assert_eq!(regions, vec![TimeSpan::new(0.0, 0.4)]);
}

#[test]
fn speech_to_the_end_closes_the_region() {
    let regions = from_scores(&[0.0, 0.7, 0.7], F, 0.5, 0.0, 0.0, 0.3);
    assert_eq!(regions.len(), 1);
    assert!((regions[0].end_s - 0.3).abs() < 1e-9);
}
