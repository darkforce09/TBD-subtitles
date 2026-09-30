use super::*;
use crate::onscreen_text::replace::verify::fixtures::{occurrence, plate, quad, rect, replaced};

#[test]
fn the_region_grows_three_quarters_of_a_line_around_the_lettering_and_its_ruby() {
    let text = replaced(
        "a",
        vec![plate(0, 9, rect(90, 90, 140, 60), [0.0, 0.0])],
        20.0,
    );
    let ruby = quad(110.0, 92.0, 150.0, 99.0);
    let occurrence = occurrence("a", "Tower", quad(101.0, 100.0, 201.0, 120.0), vec![ruby]);
    let area = sample_area(&text, &occurrence, 5, (1920, 1080)).unwrap();
    assert_eq!(area.lettering, (101.0, 92.0, 201.0, 120.0));
    assert_eq!(area.region, rect(86, 76, 130, 60));
    assert_eq!(area.line_px, 20.0);
    assert!((area.scale - 2.4).abs() < 1e-9);
}

#[test]
fn the_lettering_follows_the_plate_covering_the_frame() {
    let plates = vec![
        plate(0, 4, rect(90, 90, 140, 60), [0.0, 0.0]),
        plate(5, 9, rect(290, 90, 140, 60), [200.0, 0.0]),
    ];
    let text = replaced("a", plates, 60.0);
    let occurrence = occurrence("a", "Tower", quad(100.0, 100.0, 200.0, 120.0), Vec::new());
    let early = sample_area(&text, &occurrence, 2, (1920, 1080)).unwrap();
    let late = sample_area(&text, &occurrence, 7, (1920, 1080)).unwrap();
    assert_eq!(early.lettering, (100.0, 100.0, 200.0, 120.0));
    assert_eq!(late.lettering, (300.0, 100.0, 400.0, 120.0));
    assert_eq!(late.scale, 1.0);
}

#[test]
fn the_measured_lettering_quad_wins_over_the_tracked_box() {
    let mut text = replaced(
        "a",
        vec![plate(0, 9, rect(0, 0, 400, 200), [0.0, 0.0])],
        20.0,
    );
    text.lettering_quad = Some(quad(140.0, 60.0, 180.0, 80.0));
    let occurrence = occurrence("a-c1", "Tower", quad(100.0, 40.0, 300.0, 140.0), Vec::new());
    assert_eq!(
        keyframe_quad(&text, &occurrence),
        Some(quad(140.0, 60.0, 180.0, 80.0))
    );
    let area = sample_area(&text, &occurrence, 0, (1920, 1080)).unwrap();
    assert_eq!(area.lettering, (140.0, 60.0, 180.0, 80.0));
}

#[test]
fn the_region_is_clipped_to_the_frame_and_the_scale_to_the_pixel_budget() {
    let text = replaced(
        "a",
        vec![plate(0, 9, rect(0, 0, 1920, 1080), [0.0, 0.0])],
        4.0,
    );
    let occurrence = occurrence("a", "Wide", quad(0.0, 0.0, 1919.0, 1079.0), Vec::new());
    let area = sample_area(&text, &occurrence, 0, (1920, 1080)).unwrap();
    assert_eq!(area.region, rect(0, 0, 1920, 1080));
    let pixels = f64::from(area.region.width) * f64::from(area.region.height);
    assert!(pixels * area.scale * area.scale <= MAX_READ_PIXELS + 1.0);
    assert!(area.scale >= 1.0);
}

#[test]
fn lines_count_over_the_lettering_or_where_the_writing_and_its_ruby_were() {
    let area = SampleArea {
        region: rect(100, 100, 200, 100),
        lettering: (120.0, 120.0, 220.0, 150.0),
        line_px: 20.0,
        scale: 2.0,
    };
    assert!(over_lettering(&area, quad(40.0, 40.0, 240.0, 100.0)));
    assert!(over_writing(&area, quad(40.0, 40.0, 240.0, 100.0)));
    assert!(!over_lettering(&area, quad(40.0, 150.0, 240.0, 190.0)));
    assert!(!over_lettering(&area, quad(260.0, 40.0, 390.0, 100.0)));
    // Furigana half a line above the lettering is the writing's own; the next line's, below it,
    // is not.
    let above = quad(40.0, 20.0, 140.0, 38.0);
    assert!(!over_lettering(&area, above) && over_writing(&area, above));
    let below = quad(40.0, 102.0, 140.0, 120.0);
    assert!(!over_writing(&area, below));
    // A full-height line reaching below the lettering is the writing's own, left unerased.
    let sunk = quad(40.0, 90.0, 240.0, 122.0);
    assert!(over_lettering(&area, sunk) && over_writing(&area, sunk));
    let small_sunk = quad(40.0, 92.0, 240.0, 112.0);
    assert!(over_lettering(&area, small_sunk) && !over_writing(&area, small_sunk));
}

#[test]
fn lines_are_read_in_rows_top_to_bottom_then_left_to_right() {
    let mut found = vec![
        (quad(200.0, 52.0, 300.0, 80.0), 0.9),
        (quad(0.0, 0.0, 100.0, 30.0), 0.9),
        (quad(0.0, 50.0, 100.0, 80.0), 0.9),
        (quad(150.0, 3.0, 250.0, 31.0), 0.9),
    ];
    reading_order(&mut found);
    let lefts: Vec<(f64, f64)> = found
        .iter()
        .map(|(q, _)| (q.bounds().0, q.bounds().1))
        .collect();
    assert_eq!(
        lefts,
        vec![(0.0, 0.0), (150.0, 3.0), (0.0, 50.0), (200.0, 52.0)]
    );
}
