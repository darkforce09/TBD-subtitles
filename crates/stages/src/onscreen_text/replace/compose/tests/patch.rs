use image::Luma;

use super::*;

#[test]
fn the_mask_is_feathered_by_a_three_by_three_box() {
    let mut mask = GrayImage::new(5, 5);
    mask.put_pixel(2, 2, Luma([255]));
    let feathered = feather(&mask);
    assert!((feathered[12] - 1.0 / 9.0).abs() < 1e-6);
    assert!((feathered[6] - 1.0 / 9.0).abs() < 1e-6);
    assert_eq!(feathered[0], 0.0);
    let corner = GrayImage::from_pixel(2, 2, Luma([255]));
    assert!(feather(&corner).iter().all(|&v| (v - 1.0).abs() < 1e-6));
}

#[test]
fn the_patch_puts_lettering_over_the_plate_and_covers_mask_and_lettering() {
    let plate = RgbImage::from_pixel(6, 1, Rgb([40, 80, 120]));
    let mut mask = GrayImage::new(6, 1);
    mask.put_pixel(0, 0, Luma([255]));
    let mut lettering = vec![[0.0f32; 4]; 6];
    lettering[3] = [1.0, 1.0, 1.0, 1.0];
    lettering[4] = [0.4, 0.0, 0.0, 0.6];
    let output = patch(&plate, &mask, &lettering);
    assert_eq!(output.get_pixel(0, 0).0, [40, 80, 120, 128]);
    assert_eq!(output.get_pixel(3, 0).0, [255, 255, 255, 255]);
    assert_eq!(output.get_pixel(4, 0).0, [118, 32, 48, 153]);
    assert_eq!(output.get_pixel(5, 0).0, [40, 80, 120, 0]);
}

#[test]
fn the_preview_blends_the_patch_over_the_source() {
    let source = RgbImage::from_pixel(2, 1, Rgb([0, 0, 0]));
    let mut composed = RgbaImage::new(2, 1);
    composed.put_pixel(0, 0, Rgba([200, 100, 50, 255]));
    composed.put_pixel(1, 0, Rgba([200, 100, 50, 0]));
    let preview = preview(&composed, &source);
    assert_eq!(preview.get_pixel(0, 0).0, [200, 100, 50]);
    assert_eq!(preview.get_pixel(1, 0).0, [0, 0, 0]);
}

#[test]
fn files_appear_through_a_rename() {
    let folder = std::env::temp_dir().join(format!("compose_patch_{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("0.png");
    let image = image::DynamicImage::ImageRgba8(RgbaImage::new(3, 2));
    write_png(&path, &image).unwrap();
    assert!(path.exists());
    assert!(!folder.join("0.png.tmp").exists());
    assert_eq!(
        image::open(&path).unwrap().into_rgba8().dimensions(),
        (3, 2)
    );
    std::fs::remove_dir_all(&folder).unwrap();
}
