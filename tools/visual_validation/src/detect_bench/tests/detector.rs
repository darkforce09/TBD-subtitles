use super::*;

#[test]
fn a_1080_row_frame_pads_to_1088_and_a_padded_size_stays() {
    assert_eq!(padded_height(1080), 1088);
    assert_eq!(padded_height(1088), 1088);
    assert_eq!(padded_height(360), 384);
}

#[test]
fn a_box_reaching_into_the_padding_is_clipped_to_the_frame() {
    // A caption found in 1920 × 1088 space whose unclipped box runs into the black rows.
    let found: Quad = [
        (402.5, 1031.0),
        (1517.0, 1031.0),
        (1517.0, 1086.4),
        (402.5, 1086.4),
    ];
    assert_eq!(
        clip_quad(found, 1080),
        [
            (402.5, 1031.0),
            (1517.0, 1031.0),
            (1517.0, 1080.0),
            (402.5, 1080.0)
        ]
    );
}

#[test]
fn a_box_inside_the_frame_keeps_its_corners() {
    let found: Quad = [(10.0, 20.0), (300.0, 18.5), (301.0, 64.0), (11.0, 66.0)];
    assert_eq!(clip_quad(found, 1080), found);
}
