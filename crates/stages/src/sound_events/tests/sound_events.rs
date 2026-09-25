use super::*;

fn rows(scores: &[f32]) -> Vec<Vec<f32>> {
    scores.iter().map(|s| vec![0.0, *s]).collect()
}

const RULE: ClassRule = ClassRule {
    label: "Test",
    index: 1,
    threshold: 0.5,
    min_s: 1.0,
};

#[test]
fn a_sustained_class_becomes_one_event() {
    let w = Windowing::default();
    let got = events(
        &rows(&[0.1, 0.9, 0.8, 0.9, 0.9, 0.1, 0.1]),
        &RULE,
        &w,
        "background",
    );
    assert_eq!(got.len(), 1);
    assert!((got[0].start_s - 1.25).abs() < 1e-9, "{got:?}");
    assert!((got[0].end_s - 3.25).abs() < 1e-9, "{got:?}");
    assert_eq!(got[0].peak, 0.9);
}

#[test]
fn a_one_window_spike_is_smoothed_away() {
    let w = Windowing::default();
    assert!(events(&rows(&[0.1, 0.1, 0.95, 0.1, 0.1]), &RULE, &w, "vocals").is_empty());
}

#[test]
fn every_subtitle_class_exists_in_the_rated_set() {
    assert_eq!(
        classes::rules(classes::BACKGROUND).unwrap().len(),
        classes::BACKGROUND.len()
    );
    assert_eq!(
        classes::rules(classes::VOCALS).unwrap().len(),
        classes::VOCALS.len()
    );
    assert!(classes::index_of("Speech").is_some());
}

#[test]
fn the_mean_counts_only_windows_centred_inside() {
    let w = Windowing::default();
    // Centres at 1.0, 1.5, 2.0, 2.5.
    let m = mean_score(&rows(&[1.0, 0.0, 0.0, 1.0]), 1, &w, 1.2, 2.2);
    assert_eq!(m, 0.0);
}
