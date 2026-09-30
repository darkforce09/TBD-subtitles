use job_model::onscreen::{Point, Quad};

use super::{Claim, covered};

fn rect(l: f64, t: f64, r: f64, b: f64) -> Quad {
    Quad([
        Point { x: l, y: t },
        Point { x: r, y: t },
        Point { x: r, y: b },
        Point { x: l, y: b },
    ])
}

fn claim(frames: (u64, u64), quad: Quad, found_by_claude: bool) -> Claim {
    Claim {
        first_frame: frames.0,
        last_frame: frames.1,
        quad,
        found_by_claude,
    }
}

#[test]
fn the_detector_occurrence_of_a_duplicate_pair_is_kept() {
    let claims = [
        claim((100, 110), rect(640.0, 780.0, 1130.0, 850.0), true),
        claim((100, 140), rect(660.0, 775.0, 1150.0, 875.0), false),
    ];
    assert_eq!(covered(&claims), [true, false]);
}

#[test]
fn a_longer_then_larger_occurrence_wins_between_equals() {
    let short = claim((0, 10), rect(0.0, 0.0, 100.0, 40.0), false);
    let long = claim((0, 30), rect(10.0, 5.0, 90.0, 35.0), false);
    assert_eq!(covered(&[short, long]), [true, false]);
    let small = claim((0, 10), rect(10.0, 5.0, 90.0, 35.0), false);
    let large = claim((0, 10), rect(0.0, 0.0, 100.0, 40.0), false);
    assert_eq!(covered(&[small, large]), [true, false]);
}

#[test]
fn a_box_mostly_inside_another_is_the_same_sign() {
    let line = claim((0, 50), rect(690.0, 850.0, 1230.0, 950.0), true);
    let reading = claim((0, 50), rect(694.0, 854.0, 765.0, 881.0), true);
    assert_eq!(covered(&[line, reading]), [false, true]);
}

#[test]
fn separate_lines_and_different_times_both_stay() {
    let top = claim((0, 50), rect(768.0, 778.0, 1152.0, 875.0), false);
    let bottom = claim((0, 50), rect(672.0, 885.0, 1250.0, 983.0), false);
    assert_eq!(covered(&[top, bottom]), [false, false]);
    let earlier = claim((0, 10), rect(0.0, 0.0, 100.0, 40.0), false);
    let later = claim((11, 20), rect(0.0, 0.0, 100.0, 40.0), false);
    assert_eq!(covered(&[earlier, later]), [false, false]);
}

#[test]
fn a_chain_of_overlaps_keeps_every_uncovered_occurrence() {
    let left = claim((0, 90), rect(0.0, 0.0, 100.0, 40.0), false);
    let middle = claim((0, 50), rect(40.0, 0.0, 140.0, 40.0), false);
    let right = claim((0, 10), rect(120.0, 0.0, 220.0, 40.0), false);
    assert_eq!(covered(&[left, middle, right]), [false, true, false]);
}
