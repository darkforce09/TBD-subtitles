use super::super::fixtures::{Background, Glyphs, OUTLINE, iou, plate_of, rect_quad};
use super::{UNSEPARATED, dilation_radius, in_ring, segment};

#[test]
fn dilation_grows_with_the_line_height() {
    assert_eq!(dilation_radius(10.0), 2);
    assert_eq!(dilation_radius(100.0), 6);
    assert_eq!(dilation_radius(1e9), 255);
}

#[test]
fn the_ring_is_three_pixels_deep() {
    assert!(in_ring(0, 5, 20, 20) && in_ring(2, 5, 20, 20) && in_ring(17, 5, 20, 20));
    assert!(!in_ring(3, 3, 20, 20) && !in_ring(16, 16, 20, 20));
}

#[test]
fn ink_covering_most_of_the_quad_is_not_writing() {
    let block = Glyphs {
        strokes: vec![(200, 150, 300, 190)],
        outline: 0,
    };
    let quad = block.quad(1.0);
    let (image, plate, window) = plate_of(quad, &|x, y| block.paint(x, y, [40, 40, 40]));
    let result = segment(&image, plate, window, quad, 42.0);
    assert_eq!(result.err(), Some(UNSEPARATED));
}

#[test]
fn a_speck_in_a_large_quad_is_not_writing() {
    let speck = Glyphs {
        strokes: vec![(250, 170, 253, 173)],
        outline: 0,
    };
    let quad = rect_quad(200.0, 150.0, 330.0, 200.0);
    let (image, plate, window) = plate_of(quad, &|x, y| speck.paint(x, y, [40, 40, 40]));
    assert_eq!(
        segment(&image, plate, window, quad, 50.0).err(),
        Some(UNSEPARATED)
    );
}

#[test]
fn a_flat_region_has_no_ink() {
    let quad = rect_quad(200.0, 150.0, 330.0, 200.0);
    let (image, plate, window) = plate_of(quad, &|_, _| [90, 90, 90]);
    assert_eq!(
        segment(&image, plate, window, quad, 50.0).err(),
        Some(UNSEPARATED)
    );
}

#[test]
fn the_mask_is_zero_outside_the_dilated_window() {
    let glyphs = Glyphs::row(200, 160, 2);
    let quad = glyphs.quad(4.0);
    let (image, plate, window) = plate_of(quad, &|x, y| {
        glyphs.paint(x, y, Background::Gradient.at(x, y))
    });
    let result = segment(&image, plate, window, quad, 42.0).expect("segmented");
    assert_eq!(result.mask.dimensions(), (plate.width, plate.height));
    let r = u32::from(dilation_radius(42.0));
    for (x, y, p) in result.mask.enumerate_pixels() {
        let (fx, fy) = (plate.x + x, plate.y + y);
        let near = fx + r >= window.x
            && fx < window.right() + r
            && fy + r >= window.y
            && fy < window.bottom() + r;
        assert!(p.0[0] == 0 || near, "mask outside the window at {fx},{fy}");
    }
    assert!(iou(&result.mask, &glyphs.truth(plate, 2)) >= 0.85);
}

#[test]
fn a_fading_halo_is_a_soft_outline() {
    let glyphs = Glyphs::row(200, 160, 2);
    let quad = glyphs.quad(8.0);
    let background = [90, 140, 200];
    let halo = glyphs.clone();
    let paint = move |x: u32, y: u32| {
        if let Some(ink) = halo.at(x, y).map(|_| halo.paint(x, y, background)) {
            return ink;
        }
        for step in 1..=4u8 {
            let wider = Glyphs {
                strokes: halo.strokes.clone(),
                outline: halo.outline + i64::from(step),
            };
            if wider.at(x, y).is_some() {
                let u = f64::from(step) / 5.0;
                return std::array::from_fn(|c| {
                    (f64::from(OUTLINE[c]) * (1.0 - u) + f64::from(background[c]) * u) as u8
                });
            }
        }
        background
    };
    let (image, plate, window) = plate_of(quad, &paint);
    let result = segment(&image, plate, window, quad, 46.0).expect("segmented");
    assert!(result.style.soft_outline, "{:?}", result.style);
    let hard = glyphs.clone();
    let (image, plate, window) = plate_of(quad, &|x, y| hard.paint(x, y, background));
    let result = segment(&image, plate, window, quad, 46.0).expect("segmented");
    assert!(!result.style.soft_outline, "{:?}", result.style);
}

#[test]
fn anti_aliased_edges_still_separate() {
    let fine = Glyphs::row(401, 321, 0);
    for background in [Background::Flat, Background::Noise] {
        let coverage = |x: u32, y: u32| {
            let hits = (0..2)
                .flat_map(|i| (0..2).map(move |j| (2 * x + i, 2 * y + j)))
                .filter(|&(sx, sy)| fine.at(sx, sy).is_some())
                .count();
            hits as f64 / 4.0
        };
        let paint = |x: u32, y: u32| {
            let c = coverage(x, y);
            let bg = background.at(x, y);
            std::array::from_fn(|i| {
                (f64::from(bg[i]) * (1.0 - c) + f64::from(super::super::fixtures::FILL[i]) * c)
                    .round() as u8
            })
        };
        let quad = rect_quad(197.0, 157.0, 274.0, 179.0);
        let (image, plate, window) = plate_of(quad, &paint);
        let result = segment(&image, plate, window, quad, 22.0).expect("segmented");
        let ink = image::GrayImage::from_fn(plate.width, plate.height, |x, y| {
            image::Luma([if coverage(plate.x + x, plate.y + y) > 0.0 {
                255
            } else {
                0
            }])
        });
        let truth = imageproc::morphology::dilate(&ink, imageproc::distance_transform::Norm::L2, 2);
        let score = iou(&result.mask, &truth);
        assert!(score >= 0.85, "{background:?}: IoU {score}");
    }
}

#[test]
fn a_stroke_clipped_by_the_detector_box_is_erased_whole_within_half_a_line() {
    let glyphs = Glyphs::row(200, 160, 0);
    let quad = glyphs.quad(4.0);
    let (left, top, _, _) = glyphs.bounds();
    let mut clipped = glyphs.clone();
    // A brush stroke entering the box from the left, 15 px past the window.
    clipped
        .strokes
        .push((left - 19, top + 12, left + 6, top + 17));
    // Another reaching 26 px past it, beyond half a line yet inside the plate, apart from the
    // glyphs: joined to one, it would make that glyph writing the box cuts off.
    clipped
        .strokes
        .push((left - 30, top + 19, left + 6, top + 23));
    let (image, plate, window) = plate_of(quad, &|x, y| clipped.paint(x, y, [200, 200, 200]));
    let result = segment(&image, plate, window, quad, 42.0).expect("segmented");
    let masked = |x: i64, y: i64| {
        let (px, py) = (x - i64::from(plate.x), y - i64::from(plate.y));
        u32::try_from(px)
            .ok()
            .zip(u32::try_from(py).ok())
            .and_then(|(px, py)| result.mask.get_pixel_checked(px, py))
            .is_some_and(|p| p.0[0] == 255)
    };
    assert!(masked(left - 18, top + 14), "the clipped end is erased");
    assert!(masked(left + 12, top + 20), "the glyphs are erased");
    assert!(
        !masked(left - 28, top + 21),
        "a stroke reaching too far stays"
    );
}
