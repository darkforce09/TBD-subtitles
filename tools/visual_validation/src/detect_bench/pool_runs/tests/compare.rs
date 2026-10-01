use super::*;
use job_model::onscreen::Point;

fn rect(left: f64, top: f64, right: f64, bottom: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point { x: right, y: top },
        Point {
            x: right,
            y: bottom,
        },
        Point { x: left, y: bottom },
    ])
}

fn run() -> Regions {
    vec![
        vec![(rect(10.0, 10.0, 110.0, 40.0), 0.91)],
        vec![],
        vec![
            (rect(400.0, 1000.0, 1500.0, 1070.0), 0.88),
            (rect(20.0, 20.0, 60.0, 300.0), 0.72),
        ],
    ]
}

#[test]
fn a_run_compared_with_itself_is_identical_with_every_box_matched() {
    let comparison = compare(&run(), &run());
    assert!(comparison.identical());
    assert_eq!(comparison.frames, 3);
    assert_eq!(comparison.boxes, 3);
    assert_eq!(comparison.matched, 3);
    assert_eq!(describe(&comparison), "identical: 3 boxes on 3 frames");
}

#[test]
fn a_score_moving_in_the_last_digit_breaks_exact_equality_but_still_matches() {
    let mut shifted = run();
    shifted[0][0].1 = 0.910_000_1;
    shifted[2][0].0 = rect(401.0, 1000.0, 1500.0, 1070.0);
    let comparison = compare(&shifted, &run());
    assert!(!comparison.identical());
    assert_eq!(comparison.identical_frames, 1);
    assert_eq!(comparison.matched, 3);
    assert_eq!(
        describe(&comparison),
        "differs: 1 of 3 frames identical; 3 boxes against 3; 3 matched at IoU ≥ 0.5"
    );
}

#[test]
fn a_missing_box_and_a_distant_box_do_not_match() {
    let mut other = run();
    other[0].clear();
    other[2][1].0 = rect(900.0, 20.0, 940.0, 300.0);
    let comparison = compare(&other, &run());
    assert_eq!(comparison.boxes, 2);
    assert_eq!(comparison.reference_boxes, 3);
    assert_eq!(comparison.matched, 1);
}

#[test]
fn one_reference_box_pairs_with_one_found_box_only() {
    let reference = vec![vec![(rect(0.0, 0.0, 100.0, 100.0), 0.9)]];
    let doubled = vec![vec![
        (rect(0.0, 0.0, 100.0, 100.0), 0.9),
        (rect(2.0, 2.0, 100.0, 100.0), 0.8),
    ]];
    assert_eq!(compare(&doubled, &reference).matched, 1);
}

#[test]
fn runs_of_different_lengths_are_never_identical() {
    let short = run()[..2].to_vec();
    let comparison = compare(&short, &run());
    assert_eq!(comparison.frames, 2);
    assert_eq!(comparison.identical_frames, 2);
    assert!(!comparison.identical());
    assert!(describe(&comparison).ends_with("the runs covered different frame counts"));
}

#[test]
fn overlap_is_measured_on_bounding_rectangles() {
    let a = rect(0.0, 0.0, 10.0, 10.0);
    assert_eq!(iou(a, a), 1.0);
    assert_eq!(iou(a, rect(5.0, 0.0, 15.0, 10.0)), 50.0 / 150.0);
    assert_eq!(iou(a, rect(20.0, 20.0, 30.0, 30.0)), 0.0);
    assert_eq!(iou(rect(1.0, 1.0, 1.0, 1.0), rect(1.0, 1.0, 1.0, 1.0)), 0.0);
}
