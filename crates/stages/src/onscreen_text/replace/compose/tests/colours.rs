use image::Rgb;

use super::*;

fn style(fill: [u8; 3], outline: Option<[u8; 3]>, outline_px: f64) -> LetteringStyle {
    LetteringStyle {
        fill_rgb: fill,
        outline_rgb: outline,
        outline_px,
        soft_outline: true,
        stroke_px: 4.0,
        line_height_px: 40.0,
    }
}

#[test]
fn wcag_contrast_spans_one_to_twenty_one() {
    let white = luminance([255.0; 3]);
    let black = luminance([0.0; 3]);
    assert!((white - 1.0).abs() < 1e-9 && black.abs() < 1e-9);
    assert!((contrast(white, black) - 21.0).abs() < 1e-9);
    assert!((contrast(black, white) - 21.0).abs() < 1e-9);
    assert!((contrast(0.3, 0.3) - 1.0).abs() < 1e-9);
}

#[test]
fn low_contrast_lettering_gets_a_legibility_outline() {
    // White on pale grey: a black outline, 2 pixels, hard.
    let chosen = colours(&style([255, 255, 255], None, 0.0), [230.0, 230.0, 230.0]);
    assert_eq!(chosen.fill, [255, 255, 255]);
    assert_eq!(
        chosen.outline,
        Some(Outline {
            rgb: [0, 0, 0],
            width: 2.0,
            soft: false
        })
    );
    // Dark red on black: a white outline.
    let chosen = colours(&style([90, 0, 0], None, 0.0), [10.0, 10.0, 10.0]);
    assert_eq!(chosen.outline.unwrap().rgb, [255, 255, 255]);
}

#[test]
fn readable_lettering_keeps_no_outline_and_measured_outlines_stay() {
    let chosen = colours(&style([255, 255, 255], None, 0.0), [20.0, 20.0, 20.0]);
    assert_eq!(chosen.outline, None);
    let chosen = colours(
        &style([255, 255, 0], Some([0, 0, 90]), 0.4),
        [255.0, 255.0, 0.0],
    );
    assert_eq!(
        chosen.outline,
        Some(Outline {
            rgb: [0, 0, 90],
            width: 1.0,
            soft: true
        })
    );
}

#[test]
fn the_background_mean_is_taken_under_the_writing() {
    let mut plate = RgbImage::from_pixel(10, 10, Rgb([0, 0, 0]));
    for y in 2..5 {
        for x in 2..8 {
            plate.put_pixel(x, y, Rgb([200, 100, 50]));
        }
    }
    let quad = Quad([
        Point { x: 2.0, y: 2.0 },
        Point { x: 8.0, y: 2.0 },
        Point { x: 8.0, y: 5.0 },
        Point { x: 2.0, y: 5.0 },
    ]);
    assert_eq!(mean_under(&plate, quad), [200.0, 100.0, 50.0]);
    let outside = Quad([
        Point { x: 50.0, y: 50.0 },
        Point { x: 60.0, y: 50.0 },
        Point { x: 60.0, y: 60.0 },
        Point { x: 50.0, y: 60.0 },
    ]);
    let whole = mean_under(&plate, outside);
    assert!((whole[0] - 200.0 * 18.0 / 100.0).abs() < 1e-9);
}
