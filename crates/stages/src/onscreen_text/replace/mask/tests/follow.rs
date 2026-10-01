use super::{LEAST_DISTURBED_SCORE, MIN_SCORE, Match, Path, Placement, UNFOLLOWED, path};

const MOVED: Placement = Placement {
    dx: 4,
    dy: 0,
    scale: Placement::KEY.scale,
};

/// `frames` matches at the keyframe placement with a perfect score.
fn held(frames: usize) -> Vec<Match> {
    vec![Some((Placement::KEY, 1.0)); frames]
}

#[test]
fn every_frame_followed_is_a_moving_path() {
    let mut found = held(10);
    found[3] = Some((MOVED, 0.95));
    let mut expected = vec![(Placement::KEY, 1.0); 10];
    expected[3] = (MOVED, 0.95);
    assert_eq!(path(&found), Ok(Path::Moving(expected)));
}

#[test]
fn a_held_sign_disturbed_in_a_few_frames_is_still() {
    let mut found = held(40);
    for i in [11, 13, 17, 19] {
        found[i] = Some((MOVED, MIN_SCORE - 0.01));
    }
    assert_eq!(path(&found), Ok(Path::Still));
}

#[test]
fn writing_that_leaves_the_keyframe_placement_or_vanishes_is_not_still() {
    let mut moved = held(40);
    moved[5] = Some((MOVED, 0.95));
    moved[6] = Some((MOVED, MIN_SCORE - 0.01));
    assert_eq!(path(&moved), Err(UNFOLLOWED));

    let mut gone = held(40);
    gone[5] = Some((MOVED, LEAST_DISTURBED_SCORE - 0.01));
    assert_eq!(path(&gone), Err(UNFOLLOWED));

    let mut unplaced = held(40);
    unplaced[5] = None;
    assert_eq!(path(&unplaced), Err(UNFOLLOWED));
}

#[test]
fn a_sign_lost_in_too_many_frames_is_not_still() {
    let mut found = held(20);
    for frame in found.iter_mut().take(3) {
        *frame = Some((MOVED, MIN_SCORE - 0.1));
    }
    assert_eq!(path(&found), Err(UNFOLLOWED));
}
