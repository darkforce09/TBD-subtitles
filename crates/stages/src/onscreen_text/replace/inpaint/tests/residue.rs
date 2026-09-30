use image::{GrayImage, Luma, Rgb, RgbImage};
use job_model::onscreen::LetteringStyle;

use super::{residue_share, widened};

const PAPER: [u8; 3] = [235, 232, 225];
const INK: [u8; 3] = [25, 25, 30];

fn style(fill_rgb: [u8; 3]) -> LetteringStyle {
    LetteringStyle {
        fill_rgb,
        outline_rgb: None,
        outline_px: 0.0,
        soft_outline: false,
        stroke_px: 8.0,
        line_height_px: 40.0,
    }
}

/// The whole 80 × 60 plate masked.
fn everything() -> GrayImage {
    GrayImage::from_pixel(80, 60, Luma([255]))
}

#[test]
fn a_clean_fill_leaves_no_residue() {
    let filled = RgbImage::from_pixel(80, 60, Rgb(PAPER));
    assert_eq!(residue_share(&filled, &everything(), &style(INK)), 0.0);
}

#[test]
fn a_stroke_the_fill_left_is_residue() {
    let filled = RgbImage::from_fn(80, 60, |x, y| {
        Rgb(if (20..60).contains(&x) && (26..34).contains(&y) {
            INK
        } else {
            PAPER
        })
    });
    let share = residue_share(&filled, &everything(), &style(INK));
    let stroke = 40.0 * 8.0 / (80.0 * 60.0);
    assert!(
        (share - stroke).abs() < 0.01,
        "share {share}, stroke {stroke}"
    );
    assert!(share > super::MAX_RESIDUE_SHARE);
}

#[test]
fn a_fill_in_the_writing_colour_all_around_is_not_residue() {
    let filled = RgbImage::from_pixel(80, 60, Rgb(INK));
    assert_eq!(residue_share(&filled, &everything(), &style(INK)), 0.0);
}

#[test]
fn hairline_texture_the_fill_continues_is_not_residue() {
    let filled = RgbImage::from_fn(80, 60, |x, y| {
        Rgb(if y % 12 == 0 || x % 20 == 0 {
            INK
        } else {
            PAPER
        })
    });
    assert_eq!(residue_share(&filled, &everything(), &style(INK)), 0.0);
}

#[test]
fn only_masked_pixels_count() {
    let filled = RgbImage::from_fn(80, 60, |x, _| Rgb(if x < 40 { INK } else { PAPER }));
    let right = GrayImage::from_fn(80, 60, |x, _| Luma([if x >= 40 { 255 } else { 0 }]));
    assert_eq!(residue_share(&filled, &right, &style(INK)), 0.0);
}

#[test]
fn the_retry_mask_grows_by_a_tenth_of_a_line() {
    let mut mask = GrayImage::new(80, 60);
    mask.put_pixel(40, 30, Luma([255]));
    let wider = widened(&mask, 40.0);
    assert_eq!(wider.get_pixel(44, 30).0[0], 255);
    assert_eq!(wider.get_pixel(45, 30).0[0], 0);
    assert!(wider.pixels().all(|p| p.0[0] == 0 || p.0[0] == 255));
    let tiny = widened(&mask, 2.0);
    assert_eq!(tiny.get_pixel(41, 30).0[0], 255);
}
