use super::*;

fn word(text: &str, start_s: f64, end_s: f64) -> TimedWord {
    TimedWord {
        text: text.into(),
        start_s,
        end_s,
        confidence: None,
    }
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

#[test]
fn backbone_times_follow_matches_and_substitutions_but_not_insertions() {
    let display = strings(&["Law,", "the", "Birdcage", "is", "closing", "in!"]);
    let backbone = [
        word("law", 0.0, 0.3),
        word("the", 0.3, 0.4),
        word("bird", 0.4, 0.7),
        word("is", 0.8, 0.9),
        word("closing", 0.9, 1.3),
    ];
    let times = backbone_times(&display, &backbone);
    assert_eq!(times[0], Some((0.0, 0.3)));
    assert_eq!(times[2], Some((0.4, 0.7)));
    assert_eq!(times[4], Some((0.9, 1.3)));
    assert_eq!(times[5], None);
}

#[test]
fn untimed_words_are_spread_between_their_neighbours_by_length() {
    let words = strings(&["a", "bb", "c", "dddd"]);
    let times = [Some((1.0, 1.5)), None, None, Some((2.5, 3.0))];
    let filled = interpolate(&words, &times, 0.0, 10.0);
    assert_eq!(filled[0], (1.0, 1.5));
    assert!((filled[1].0 - 1.5).abs() < 1e-9 && (filled[1].1 - 1.5 - 2.0 / 3.0).abs() < 1e-9);
    assert!((filled[2].1 - 2.5).abs() < 1e-9);
    let edges = interpolate(&strings(&["x", "y"]), &[None, None], 4.0, 6.0);
    assert_eq!(edges, vec![(4.0, 5.0), (5.0, 6.0)]);
}

#[test]
fn an_alignment_inside_its_window_near_the_backbone_passes() {
    let aligned = [Some((1.0, 1.2)), Some((1.3, 1.6)), Some((1.7, 2.0))];
    let reference = [Some((1.05, 1.2)), None, Some((1.6, 2.0))];
    assert!(passes(&[(&aligned, (1.0, 2.0), &reference)]));
}

#[test]
fn an_alignment_fails_outside_its_window_far_from_the_backbone_or_collapsed() {
    let reference = [Some((1.0, 1.2)), Some((1.3, 1.6)), Some((1.7, 2.0))];
    let late = [Some((3.5, 3.6)), Some((3.6, 3.8)), Some((3.8, 4.0))];
    assert!(!passes(&[(&late, (1.0, 2.0), &reference)]));
    let shifted = [Some((1.5, 1.6)), Some((1.8, 1.9)), Some((2.2, 2.4))];
    assert!(!passes(&[(&shifted, (1.0, 2.0), &reference)]));
    let flat = [Some((1.0, 1.0)), Some((1.0, 1.0)), Some((1.0, 1.0))];
    assert!(!passes(&[(&flat, (1.0, 2.0), &[None, None, None])]));
    let empty: [Option<(f64, f64)>; 1] = [None];
    assert!(!passes(&[(&empty, (1.0, 2.0), &[None])]));
}

#[test]
fn the_signed_median_keeps_the_direction() {
    assert_eq!(signed_median(&mut []), None);
    assert_eq!(signed_median(&mut [0.08, -0.02, 0.04]), Some(0.04));
    assert_eq!(signed_median(&mut [-0.1, -0.3]), Some(-0.2));
}
