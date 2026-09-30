use super::*;

fn candidate(first: u64, last: u64, plate_starts: Vec<u64>, left: f64) -> Candidate {
    Candidate {
        first_frame: first,
        last_frame: last,
        plate_starts,
        area: (left, 0.0, left + 100.0, 40.0),
    }
}

#[test]
fn a_lone_occurrence_checks_its_ends_middle_and_plate_starts() {
    let alone = candidate(100, 200, vec![100, 130, 170], 0.0);
    assert_eq!(sample_frames(&alone, &[]), vec![100, 130, 150, 170, 200]);
}

#[test]
fn a_one_frame_occurrence_checks_that_frame_once() {
    let short = candidate(5, 5, vec![5], 0.0);
    assert_eq!(sample_frames(&short, &[]), vec![5]);
}

#[test]
fn an_overlapping_replacement_adds_the_frames_around_its_ends() {
    let target = candidate(100, 200, vec![100], 0.0);
    let over = candidate(150, 260, vec![150], 10.0);
    let apart = candidate(120, 180, vec![120], 500.0);
    let later = candidate(202, 300, vec![202], 0.0);
    assert_eq!(
        sample_frames(&target, &[&over, &apart, &later]),
        vec![100, 149, 150, 200]
    );
    let touching = candidate(201, 300, vec![201], 0.0);
    assert_eq!(sample_frames(&target, &[&touching]), vec![100, 150, 200]);
}

#[test]
fn many_plates_are_spread_evenly_up_to_the_cap() {
    let moving = candidate(0, 1000, (0..1000).step_by(10).collect(), 0.0);
    let frames = sample_frames(&moving, &[]);
    assert_eq!(frames.len(), MAX_SAMPLES);
    assert!(frames.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(frames.contains(&0) && frames.contains(&500) && frames.contains(&1000));
    assert!(frames.contains(&10) && frames.contains(&990));
}

#[test]
fn iou_measures_shared_area_over_the_union() {
    let a = (0.0, 0.0, 10.0, 10.0);
    assert_eq!(iou(a, a), 1.0);
    assert_eq!(iou(a, (20.0, 0.0, 30.0, 10.0)), 0.0);
    assert!((iou(a, (5.0, 0.0, 15.0, 10.0)) - 1.0 / 3.0).abs() < 1e-9);
}
