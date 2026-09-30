use image::{GrayImage, Luma, Rgb, RgbImage};

use super::{Integral, Plane, Template, grey};

fn pattern(w: u32, h: u32, offset: u32) -> GrayImage {
    GrayImage::from_fn(w, h, |x, y| {
        Luma([if ((x + offset) / 3 + y / 4).is_multiple_of(2) {
            40
        } else {
            200
        }])
    })
}

#[test]
fn an_exact_match_scores_one_regardless_of_brightness() {
    let image = Plane::from_gray(&pattern(30, 20, 0));
    let integral = Integral::new(&image);
    let template_image = GrayImage::from_fn(9, 8, |x, y| {
        let v = pattern(30, 20, 0).get_pixel(x + 6, y + 4).0[0];
        Luma([v / 2 + 10])
    });
    let template = Template::new(&Plane::from_gray(&template_image)).expect("contrast");
    assert!((template.score(&image, &integral, 6, 4) - 1.0).abs() < 1e-4);
    assert!(template.score(&image, &integral, 7, 4) < 0.9);
}

#[test]
fn flat_templates_and_windows_do_not_match() {
    let flat = Plane::from_gray(&GrayImage::from_pixel(10, 10, Luma([90])));
    assert!(Template::new(&flat).is_none());
    let template = Template::new(&Plane::from_gray(&pattern(6, 6, 0))).expect("contrast");
    assert_eq!(template.score(&flat, &Integral::new(&flat), 2, 2), 0.0);
}

#[test]
fn shrinking_averages_blocks() {
    let plane = Plane::from_gray(&GrayImage::from_fn(4, 4, |x, y| {
        Luma([(x * 10 + y * 40) as u8])
    }));
    let small = plane.shrink(2);
    assert_eq!((small.w, small.h), (2, 2));
    assert_eq!(small.data, [25.0, 45.0, 105.0, 125.0]);
}

#[test]
fn grey_uses_rec601_weights() {
    let image = RgbImage::from_pixel(1, 1, Rgb([255, 0, 0]));
    assert_eq!(grey(&image).get_pixel(0, 0).0[0], 76);
}
