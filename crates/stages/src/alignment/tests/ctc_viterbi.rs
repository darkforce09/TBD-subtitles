use super::*;

const BLANK: usize = 3;

/// A grid where `peaks[t]` is the likely token at frame `t`.
fn grid(peaks: &[usize]) -> Vec<f32> {
    let mut out = Vec::new();
    for &peak in peaks {
        for v in 0..4 {
            out.push(if v == peak { -0.1 } else { -5.0 });
        }
    }
    out
}

#[test]
fn tokens_land_on_their_peaks() {
    let g = grid(&[3, 3, 0, 0, 3, 3, 1, 3, 2, 2]);
    let spans = align(&g, 4, &[0, 1, 2], BLANK).unwrap();
    assert_eq!(spans, vec![(2, 4), (6, 7), (8, 10)]);
}

#[test]
fn a_repeated_token_needs_a_blank_between() {
    let g = grid(&[0, 0, 3, 0, 0]);
    let spans = align(&g, 4, &[0, 0], BLANK).unwrap();
    assert!(spans[0].1 <= 2 && spans[1].0 >= 3, "{spans:?}");
}

#[test]
fn too_few_frames_is_no_alignment() {
    let g = grid(&[0, 3]);
    assert_eq!(align(&g, 4, &[0, 0], BLANK), None);
}

#[test]
fn every_token_gets_a_frame_in_order() {
    let g = grid(&[3; 12]);
    let spans = align(&g, 4, &[0, 1, 2, 0], BLANK).unwrap();
    for pair in spans.windows(2) {
        assert!(pair[0].1 <= pair[1].0);
    }
    assert!(spans.iter().all(|(a, b)| b > a));
}
