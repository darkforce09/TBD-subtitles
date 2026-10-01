use image::{GrayImage, Luma, RgbImage};

use super::same_background;

fn flat(value: u8) -> RgbImage {
    RgbImage::from_pixel(20, 10, image::Rgb([value; 3]))
}

#[test]
fn small_even_changes_keep_the_background() {
    let mask = GrayImage::new(20, 10);
    assert!(same_background(&flat(100), &flat(102), &mask));
    assert!(!same_background(&flat(100), &flat(104), &mask));
}

#[test]
fn a_few_strong_changes_start_a_new_run() {
    let mask = GrayImage::new(20, 10);
    let mut changed = flat(100);
    for x in 0..4 {
        changed.put_pixel(x, 0, image::Rgb([200; 3]));
    }
    assert!(
        !same_background(&flat(100), &changed, &mask),
        "2 % of pixels far off"
    );
    let reference = RgbImage::from_pixel(40, 10, image::Rgb([100; 3]));
    let mut speck = reference.clone();
    speck.put_pixel(0, 0, image::Rgb([140; 3]));
    let mask = GrayImage::new(40, 10);
    assert!(
        same_background(&reference, &speck, &mask),
        "0.25 % of pixels off"
    );
}

#[test]
fn masked_pixels_are_ignored() {
    let mut mask = GrayImage::new(20, 10);
    let mut changed = flat(100);
    for x in 0..20 {
        for y in 0..5 {
            mask.put_pixel(x, y, Luma([255]));
            changed.put_pixel(x, y, image::Rgb([0; 3]));
        }
    }
    assert!(same_background(&flat(100), &changed, &mask));
    let all = GrayImage::from_pixel(20, 10, Luma([255]));
    assert!(
        same_background(&flat(0), &flat(255), &all),
        "nothing left to compare"
    );
}
