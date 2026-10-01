use image::{ImageFormat, Rgb, RgbImage};

use super::*;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-png-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_written_png_reads_back_and_leaves_no_temporary_file() {
    let dir = folder("written");
    let path = dir.join("crop.png");
    let image = RgbImage::from_pixel(3, 2, Rgb([10, 20, 30]));
    write(&path, |out| image.write_to(out, ImageFormat::Png)).unwrap();
    let back = image::open(&path).unwrap().to_rgb8();
    assert_eq!(back, image);
    assert!(!dir.join("crop.png.partial").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_failed_encoding_leaves_neither_the_file_nor_its_temporary() {
    let dir = folder("failed");
    let path = dir.join("crop.png");
    let error = write(&path, |_| {
        Err(image::ImageError::IoError(std::io::Error::other(
            "the encoder stopped",
        )))
    })
    .unwrap_err();
    assert!(error.to_string().contains("the encoder stopped"), "{error}");
    assert!(!path.exists());
    assert!(!dir.join("crop.png.partial").exists());
    fs::remove_dir_all(&dir).unwrap();
}
