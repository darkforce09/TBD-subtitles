use super::*;

#[test]
fn rows_fold_into_runs_of_one_shift_per_plate() {
    let mut motion = Motion::default();
    for frame in 10..=12 {
        motion.add("a", frame, 0, [0.0, 0.0]);
    }
    motion.add("a", 13, 0, [1.0, 0.0]);
    motion.add("a", 14, 0, [0.0, 0.0]);
    motion.add("a", 15, 1, [4.0, 0.0]);
    motion.add("a", 16, 1, [4.0, 0.0]);
    motion.add("b", 3, 0, [0.0, 0.0]);
    assert_eq!(
        motion.runs("a", 0),
        [
            ShiftRun {
                first_frame: 10,
                last_frame: 12,
                shift: [0.0, 0.0]
            },
            ShiftRun {
                first_frame: 13,
                last_frame: 13,
                shift: [1.0, 0.0]
            },
            ShiftRun {
                first_frame: 14,
                last_frame: 14,
                shift: [0.0, 0.0]
            },
        ]
    );
    assert_eq!(motion.runs("a", 1).len(), 1);
    assert_eq!(motion.runs("b", 0).len(), 1);
    assert!(motion.runs("c", 0).is_empty());
    assert!(motion.runs("a", 7).is_empty());
}

#[test]
fn a_frame_takes_the_shift_of_its_run_and_none_outside_every_run() {
    let mut motion = Motion::default();
    motion.add("a", 10, 0, [0.0, 0.0]);
    motion.add("a", 11, 0, [2.0, 1.0]);
    motion.add("a", 13, 0, [2.0, 1.0]);
    assert_eq!(motion.shift_at("a", 0, 10), Some([0.0, 0.0]));
    assert_eq!(motion.shift_at("a", 0, 11), Some([2.0, 1.0]));
    assert_eq!(motion.shift_at("a", 0, 12), None);
    assert_eq!(motion.shift_at("a", 0, 13), Some([2.0, 1.0]));
    assert_eq!(motion.shift_at("a", 0, 9), None);
    assert_eq!(motion.shift_at("a", 1, 10), None);
}

#[test]
fn other_shifts_are_listed_once_in_first_use_order() {
    let mut motion = Motion::default();
    for (frame, shift) in [(1, 0.0), (2, 1.0), (3, 0.0), (4, 2.0), (5, 1.0)] {
        motion.add("a", frame, 0, [shift, 0.0]);
    }
    assert_eq!(
        motion.other_shifts("a", 0, [0.0, 0.0]),
        vec![[1.0, 0.0], [2.0, 0.0]]
    );
    assert!(motion.other_shifts("a", 1, [0.0, 0.0]).is_empty());
    assert!(!motion.is_empty());
    assert!(Motion::default().is_empty());
}
