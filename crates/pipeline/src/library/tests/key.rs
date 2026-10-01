use super::*;
use crate::library::fixtures::sign;
use image::{DynamicImage, Luma};

#[test]
fn normalising_folds_width_forms_and_drops_whitespace() {
    assert_eq!(normalised("ﾄﾞﾚｽﾛｰｻﾞ　王宮"), "ドレスローザ王宮");
    assert_eq!(normalised(" 海 \n軍\t"), "海軍");
    assert_eq!(normalised("ＳＭＩＬＥ１"), "SMILE1");
    assert_eq!(normalised("   "), "");
}

#[test]
fn an_identical_crop_hashes_the_same() {
    let picture = DynamicImage::ImageLuma8(sign(7));
    assert_eq!(
        distance(difference_hash(&picture), difference_hash(&picture.clone())),
        0
    );
}

#[test]
fn a_one_pixel_change_moves_the_hash_little() {
    let original = sign(7);
    let mut touched = original.clone();
    let Luma([value]) = *touched.get_pixel(40, 20);
    touched.put_pixel(40, 20, Luma([255 - value]));
    let apart = distance(
        difference_hash(&DynamicImage::ImageLuma8(original)),
        difference_hash(&DynamicImage::ImageLuma8(touched)),
    );
    assert!(apart <= 2, "{apart} bits apart");
}

#[test]
fn a_different_sign_hashes_far_apart() {
    let apart = distance(
        difference_hash(&DynamicImage::ImageLuma8(sign(7))),
        difference_hash(&DynamicImage::ImageLuma8(sign(8))),
    );
    assert!(apart > 3 * MATCH_DISTANCE, "{apart} bits apart");
}

#[test]
fn a_crop_file_hashes_as_its_pixels_and_a_broken_one_is_an_error() {
    let folder = std::env::temp_dir().join(format!("tbd-library-key-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let crop = folder.join("crop.png");
    sign(3).save(&crop).unwrap();
    assert_eq!(
        crop_hash(&crop).unwrap(),
        difference_hash(&DynamicImage::ImageLuma8(sign(3)))
    );
    let broken = folder.join("broken.png");
    std::fs::write(&broken, b"not a picture").unwrap();
    assert!(crop_hash(&broken).is_err());
    assert!(crop_hash(&folder.join("missing.png")).is_err());
    let _ = std::fs::remove_dir_all(&folder);
}
