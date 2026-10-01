use super::*;

/// Probability maps for `shape` with one 0.9 block per image, `[left, top, right, bottom)`.
fn maps(shape: InputShape, blocks: &[[usize; 4]]) -> Vec<f32> {
    let mut maps = vec![0.0; shape.output_len()];
    for (image, &[left, top, right, bottom]) in blocks.iter().enumerate() {
        for y in top..bottom {
            for x in left..right {
                maps[image * shape.plane() + y * shape.width + x] = 0.9;
            }
        }
    }
    maps
}

#[test]
fn corners_are_clipped_into_the_frame() {
    let quad = clip_corners(
        [(-4.0, 2.0), (70.0, 2.0), (70.0, 50.0), (-4.0, 50.0)],
        64,
        40,
    );
    assert_eq!(quad.bounds(), (0.0, 2.0, 64.0, 40.0));
}

#[test]
fn a_region_only_in_the_padding_is_dropped() {
    let padding = [
        (10.0, 1082.0),
        (90.0, 1082.0),
        (90.0, 1087.0),
        (10.0, 1087.0),
    ];
    assert_eq!(region(padding, 0.9, 0.3, 1920, 1080), None);
}

#[test]
fn scores_below_the_minimum_or_not_finite_are_dropped_and_others_clamped() {
    let corners = [(1.0, 1.0), (9.0, 1.0), (9.0, 5.0), (1.0, 5.0)];
    assert_eq!(region(corners, 0.29, 0.3, 64, 64), None);
    assert_eq!(region(corners, f32::NAN, 0.3, 64, 64), None);
    let (_, score) = region(corners, 1.5, 0.3, 64, 64).unwrap();
    assert_eq!(score, 1.0);
}

#[test]
fn maps_become_regions_in_frame_pixels_per_image() {
    let shape = InputShape::for_frames(3, 64, 40);
    let maps = maps(shape, &[[10, 10, 40, 20], [8, 30, 48, 60]]);
    let found = PostProcess::screening()
        .regions(&maps, shape, 2, (64, 40))
        .unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].len(), 1);
    let (left, top, right, bottom) = found[0][0].0.bounds();
    assert!(left < 12.0 && right > 38.0 && top < 12.0 && bottom > 18.0);
    assert!(right < 50.0 && bottom < 30.0);
    assert!(found[0][0].1 >= 0.45);
    assert_eq!(found[1].len(), 1);
    let (_, top, _, bottom) = found[1][0].0.bounds();
    assert!(top < 32.0);
    assert_eq!(bottom, 40.0);
}

#[test]
fn confirmation_needs_the_higher_score() {
    let shape = InputShape::for_frames(1, 64, 64);
    let mut weak = maps(shape, &[[10, 10, 40, 20]]);
    weak.iter_mut()
        .filter(|value| **value > 0.0)
        .for_each(|value| *value = 0.47);
    let screened = PostProcess::screening()
        .regions(&weak, shape, 1, (64, 64))
        .unwrap();
    let confirmed = PostProcess::confirming()
        .regions(&weak, shape, 1, (64, 64))
        .unwrap();
    assert_eq!(screened[0].len(), 1);
    assert!(confirmed[0].is_empty());
}

#[test]
fn maps_of_the_wrong_size_are_an_error() {
    let shape = InputShape::for_frames(2, 32, 32);
    let short = vec![0.0; shape.output_len() - 1];
    assert!(
        PostProcess::screening()
            .regions(&short, shape, 1, (32, 32))
            .is_err()
    );
}
