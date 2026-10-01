use super::*;
use crate::onscreen_text::detect::fixtures::{timeline, yuv_frame};

/// The sample at `index` of a 24 fps scripted video, shared as the scan shares it.
fn sample(index: u64) -> Arc<YuvFrame> {
    Arc::new(yuv_frame(&[], index, false, &timeline(index + 1)))
}

fn frame_bytes() -> usize {
    sample(0).data.len()
}

fn held(candidates: Candidates, wanted: &[u64]) -> Vec<u64> {
    candidates.into_frames(wanted).into_keys().collect()
}

#[test]
fn an_active_window_keeps_every_sample_that_can_still_be_nearest_its_middle() {
    let mut candidates = Candidates::new(usize::MAX);
    let samples: Vec<Arc<YuvFrame>> = (0..=10).map(|n| sample(n * 12)).collect();
    for frame in &samples {
        candidates.offer(0, frame);
    }
    // Started at 0 s and seen at 5 s: its middle lies at or after 2.5 s.
    candidates.prune(0, 2.5);
    let kept: Vec<u64> = candidates.windows[0]
        .entries
        .iter()
        .map(|&(index, _)| index)
        .collect();
    assert_eq!(
        kept,
        [60, 72, 84, 96, 108, 120],
        "from the last at or before 2.5 s"
    );
    assert_eq!(candidates.bytes, 6 * frame_bytes());
    candidates.choose(0, 72);
    assert_eq!(
        candidates.bytes,
        frame_bytes(),
        "only the chosen sample stays"
    );
    assert_eq!(held(candidates, &[72]), [72]);
}

#[test]
fn windows_share_frames_and_release_each_only_when_no_window_holds_it() {
    let mut candidates = Candidates::new(usize::MAX);
    let (a, b) = (sample(12), sample(24));
    candidates.offer(0, &a);
    candidates.offer(1, &a);
    candidates.offer(1, &b);
    assert_eq!(candidates.bytes, 2 * frame_bytes());
    candidates.choose(0, 12);
    candidates.choose(1, 24);
    assert_eq!(candidates.bytes, 2 * frame_bytes());
    assert_eq!(held(candidates, &[12, 24]), [12, 24]);
}

#[test]
fn a_keyframe_missing_from_its_window_falls_back_to_the_video() {
    let mut candidates = Candidates::new(usize::MAX);
    candidates.offer(0, &sample(12));
    candidates.choose(0, 36);
    assert_eq!(candidates.windows[0].state, State::Fallback);
    assert_eq!(candidates.bytes, 0);
    assert!(held(candidates, &[36]).is_empty());
}

#[test]
fn over_budget_the_largest_active_window_falls_back_first() {
    let mut candidates = Candidates::new(4 * frame_bytes());
    for n in 0..3 {
        candidates.offer(0, &sample(n * 12));
    }
    candidates.offer(1, &sample(100));
    candidates.offer(1, &sample(112));
    assert_eq!(
        candidates.windows[0].state,
        State::Fallback,
        "three samples"
    );
    assert_eq!(candidates.windows[1].state, State::Open);
    assert_eq!(candidates.bytes, 2 * frame_bytes());
    assert!(candidates.bytes <= candidates.budget);
    assert_eq!(candidates.peak_bytes(), 4 * frame_bytes());
    candidates.offer(0, &sample(124));
    assert!(
        candidates.windows[0].entries.is_empty(),
        "a fallen-back window collects nothing"
    );
}

#[test]
fn a_budget_full_of_kept_keyframes_makes_a_new_occurrence_fall_back() {
    let mut candidates = Candidates::new(2 * frame_bytes());
    for (occurrence, index) in [(0, 24), (1, 12)] {
        candidates.offer(occurrence, &sample(index));
        candidates.choose(occurrence, index);
    }
    candidates.offer(2, &sample(48));
    candidates.choose(2, 48);
    assert_eq!(candidates.windows[0].state, State::Chosen(24));
    assert_eq!(candidates.windows[1].state, State::Chosen(12));
    assert_eq!(candidates.windows[2].state, State::Fallback);
    assert_eq!(held(candidates, &[12, 24, 48]), [12, 24]);
}

#[test]
fn a_zero_budget_holds_nothing() {
    let mut candidates = Candidates::new(0);
    candidates.offer(0, &sample(0));
    candidates.offer(0, &sample(12));
    candidates.choose(0, 0);
    assert_eq!(candidates.peak_bytes(), 0);
    assert!(held(candidates, &[0]).is_empty());
}
