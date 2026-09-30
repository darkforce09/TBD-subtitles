//! Separation of writing from the backgrounds real keyframes put it on: outlined lettering on a
//! translucent card over a busy picture, numerals on a small sign, furigana over a line, and the
//! pictures and cut-off writing that must still fall back.

use image::{GrayImage, Luma};
use imageproc::distance_transform::Norm;
use imageproc::morphology::dilate;
use job_model::onscreen::PixelRect;

use super::super::cluster::{distance, lab};
use super::super::fixtures::{
    Card, FILL, Glyphs, OUTLINE, busy, iou, layered, plate_of, rect_quad,
};
use super::{Segmentation, UNSEPARATED, dilation_radius, segment};

/// The name card of the fixtures: a translucent panel over the busy picture.
const CARD: Card = Card {
    rect: (150, 120, 420, 290),
};

/// Pixels of `rect` (frame coordinates) that `paint` marks, dilated by `radius`.
fn truth(rect: PixelRect, radius: u8, marked: &dyn Fn(u32, u32) -> bool) -> GrayImage {
    let ink = GrayImage::from_fn(rect.width, rect.height, |x, y| {
        Luma([u8::from(marked(rect.x + x, rect.y + y)) * 255])
    });
    dilate(&ink, Norm::L2, radius)
}

/// Share of the mask's pixels that lie within `slack` pixels of the drawn writing.
fn precision(
    mask: &GrayImage,
    rect: PixelRect,
    slack: u8,
    marked: &dyn Fn(u32, u32) -> bool,
) -> f64 {
    let near = truth(rect, slack, marked);
    let masked = mask.pixels().filter(|p| p.0[0] > 0).count();
    let inside = mask
        .pixels()
        .zip(near.pixels())
        .filter(|(m, n)| m.0[0] > 0 && n.0[0] > 0)
        .count();
    inside as f64 / masked.max(1) as f64
}

fn masked(result: &Segmentation, plate: PixelRect, x: u32, y: u32) -> bool {
    result.mask.get_pixel(x - plate.x, y - plate.y).0[0] > 0
}

fn delta_e(a: [u8; 3], b: [u8; 3]) -> f32 {
    distance(lab(a), lab(b))
}

#[test]
fn outlined_lettering_on_a_translucent_card_over_a_busy_picture_separates() {
    let line = Glyphs::row(200, 170, 3);
    let quad = line.quad(4.0);
    let paint = |x, y| layered(&[&line], x, y, CARD.over(x, y, busy(x, y)));
    let (image, plate, window) = plate_of(quad, &paint);
    let height = 44.0;
    let result = segment(&image, plate, window, quad, height).expect("separated");
    let radius = dilation_radius(height);
    let drawn = |x, y| line.at(x, y).is_some();
    let score = iou(&result.mask, &truth(plate, radius, &drawn));
    assert!(score >= 0.8, "mask IoU {score}");
    let kept = precision(&result.mask, plate, radius + 2, &drawn);
    assert!(kept >= 0.97, "the card is erased: {kept}");
    assert!(
        delta_e(result.style.fill_rgb, FILL) <= 10.0,
        "{:?}",
        result.style
    );
    let outline = result.style.outline_rgb.expect("an outline");
    assert!(delta_e(outline, OUTLINE) <= 10.0, "{:?}", result.style);
}

#[test]
fn a_two_line_card_erases_each_line_with_its_furigana_and_keeps_the_card() {
    let first = Glyphs::row(200, 170, 3);
    let furigana = Glyphs::small_row(208, 143, 4, 2);
    let second = Glyphs::row(230, 230, 3);
    let paint = |x, y| {
        layered(
            &[&first, &furigana, &second],
            x,
            y,
            CARD.over(x, y, busy(x, y)),
        )
    };
    let height = 44.0;
    let radius = dilation_radius(height);

    let quad = first.quad(4.0);
    let (image, plate, window) = plate_of(quad, &paint);
    let top = segment(&image, plate, window, quad, height).expect("first line separated");
    let drawn = |x, y| first.at(x, y).is_some() || furigana.at(x, y).is_some();
    let score = iou(&top.mask, &truth(plate, radius, &drawn));
    assert!(score >= 0.8, "first line IoU {score}");
    for c in 0..4 {
        assert!(
            masked(&top, plate, 208 + 14 * c + 5, 149),
            "furigana {c} is erased"
        );
    }
    let kept = precision(&top.mask, plate, radius + 2, &drawn);
    assert!(
        kept >= 0.97,
        "the card is erased around the first line: {kept}"
    );

    let quad = second.quad(4.0);
    let (image, plate, window) = plate_of(quad, &paint);
    let bottom = segment(&image, plate, window, quad, height).expect("second line separated");
    let drawn = |x, y| second.at(x, y).is_some();
    let score = iou(&bottom.mask, &truth(plate, radius, &drawn));
    assert!(score >= 0.8, "second line IoU {score}");
    let kept = precision(&bottom.mask, plate, radius + 2, &drawn);
    assert!(
        kept >= 0.97,
        "the second line's mask reaches past it: {kept}"
    );
}

/// Blocky "3" and "5" digits, 18 × 30 with 5-pixel strokes, starting at `(left, top)`.
fn thirty_five(left: i64, top: i64) -> Glyphs {
    let (a, b) = (left, left + 24);
    Glyphs {
        strokes: vec![
            (a, top, a + 18, top + 5),
            (a, top + 12, a + 18, top + 17),
            (a, top + 25, a + 18, top + 30),
            (a + 13, top, a + 18, top + 30),
            (b, top, b + 18, top + 5),
            (b, top + 12, b + 18, top + 17),
            (b, top + 25, b + 18, top + 30),
            (b, top, b + 5, top + 17),
            (b + 13, top + 12, b + 18, top + 30),
        ],
        outline: 0,
    }
}

#[test]
fn small_numerals_on_a_sign_filling_the_box_separate_and_the_sign_stays() {
    let digits = thirty_five(305, 157);
    let sign = (300, 140, 352, 204);
    let paint = |x: u32, y: u32| {
        if digits.at(x, y).is_some() {
            return [25, 25, 28];
        }
        let (l, t, r, b) = sign;
        if x >= l && x < r && y >= t && y < b {
            [165, 168, 170]
        } else if x % 20 < 3 {
            [55, 58, 60]
        } else {
            [20, 22, 26]
        }
    };
    let quad = rect_quad(300.0, 140.0, 352.0, 204.0);
    let (image, plate, window) = plate_of(quad, &paint);
    let result = segment(&image, plate, window, quad, 52.0).expect("separated");
    for (x, y) in [(308, 159), (320, 175), (331, 159), (345, 180)] {
        assert!(
            masked(&result, plate, x, y),
            "digit pixel {x},{y} is erased"
        );
    }
    for (x, y) in [(302, 143), (350, 200), (326, 195), (295, 170), (360, 150)] {
        assert!(!masked(&result, plate, x, y), "sign or cage {x},{y} stays");
    }
    assert!(
        delta_e(result.style.fill_rgb, [25, 25, 28]) <= 10.0,
        "{:?}",
        result.style
    );
}

#[test]
fn writing_the_box_cuts_off_falls_back() {
    let brush = Glyphs {
        strokes: vec![
            (100, 150, 400, 162),
            (200, 60, 212, 300),
            (232, 136, 246, 146),
        ],
        outline: 0,
    };
    let quad = rect_quad(180.0, 130.0, 260.0, 180.0);
    let (image, plate, window) = plate_of(quad, &|x, y| brush.paint(x, y, [200, 200, 200]));
    assert_eq!(
        segment(&image, plate, window, quad, 50.0).err(),
        Some(UNSEPARATED)
    );
}

#[test]
fn a_picture_filling_the_box_is_not_writing() {
    let quad = rect_quad(100.0, 60.0, 500.0, 300.0);
    let (image, plate, window) = plate_of(quad, &|x, y| busy(x, y));
    assert_eq!(
        segment(&image, plate, window, quad, 240.0).err(),
        Some(UNSEPARATED)
    );
    let quad = rect_quad(160.0, 130.0, 400.0, 270.0);
    let (image, plate, window) = plate_of(quad, &|x, y| CARD.over(x, y, busy(x, y)));
    assert_eq!(
        segment(&image, plate, window, quad, 140.0).err(),
        Some(UNSEPARATED)
    );
}
