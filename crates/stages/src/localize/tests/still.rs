use image::{Rgb, RgbImage, Rgba, RgbaImage};
use job_model::onscreen::PixelRect;

use super::*;
use crate::localize::colour::{Matrix, Range};

const CONVERSION: Conversion = Conversion {
    matrix: Matrix::Bt709,
    range: Range::Limited,
    bits: 10,
};

fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
    PixelRect {
        x,
        y,
        width,
        height,
    }
}

fn close(a: [u8; 3], b: [u8; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 2)
}

#[test]
fn the_even_region_grows_to_even_edges_inside_the_frame() {
    assert_eq!(
        even_region(rect(3, 5, 10, 6), (100, 50)),
        Some(rect(2, 4, 12, 8))
    );
    assert_eq!(
        even_region(rect(90, 40, 20, 20), (99, 51)),
        Some(rect(90, 40, 8, 10))
    );
    assert_eq!(even_region(rect(120, 10, 4, 4), (100, 50)), None);
}

#[test]
fn a_region_without_patches_keeps_its_colours() {
    let crop = RgbImage::from_fn(8, 6, |x, _| {
        if x < 4 {
            Rgb([30, 90, 200])
        } else {
            Rgb([240, 240, 10])
        }
    });
    let out = finished_region(&crop, rect(10, 20, 8, 6), &[], CONVERSION).unwrap();
    for (a, b) in out.pixels().zip(crop.pixels()) {
        assert!(close(a.0, b.0), "{a:?} against {b:?}");
    }
}

#[test]
fn only_the_part_of_a_patch_inside_the_region_is_blended() {
    let crop = RgbImage::from_pixel(8, 8, Rgb([0, 0, 0]));
    let white = RgbaImage::from_pixel(6, 6, Rgba([255, 255, 255, 255]));
    let clear = RgbaImage::from_pixel(4, 4, Rgba([255, 0, 0, 0]));
    let patches = [
        PlacedPatch {
            image: &white,
            rect: rect(6, 6, 6, 6),
        },
        PlacedPatch {
            image: &clear,
            rect: rect(10, 10, 4, 4),
        },
    ];
    let out = finished_region(&crop, rect(10, 10, 8, 8), &patches, CONVERSION).unwrap();
    for (x, y, pixel) in out.enumerate_pixels() {
        let expected = if x < 2 && y < 2 {
            [255, 255, 255]
        } else {
            [0, 0, 0]
        };
        assert!(close(pixel.0, expected), "({x}, {y}) is {pixel:?}");
    }
}

#[test]
fn half_covered_pixels_mix_the_patch_and_the_frame() {
    let crop = RgbImage::from_pixel(2, 2, Rgb([0, 0, 0]));
    let grey = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 128]));
    let patches = [PlacedPatch {
        image: &grey,
        rect: rect(0, 0, 2, 2),
    }];
    let out = finished_region(&crop, rect(0, 0, 2, 2), &patches, CONVERSION).unwrap();
    assert!(close(out.get_pixel(0, 0).0, [128, 128, 128]));
}

#[test]
fn odd_regions_and_mismatched_crops_are_refused() {
    let crop = RgbImage::new(4, 4);
    assert!(finished_region(&crop, rect(1, 0, 4, 4), &[], CONVERSION).is_err());
    assert!(finished_region(&crop, rect(0, 0, 4, 6), &[], CONVERSION).is_err());
    let odd = RgbImage::new(3, 4);
    assert!(finished_region(&odd, rect(0, 0, 3, 4), &[], CONVERSION).is_err());
}
