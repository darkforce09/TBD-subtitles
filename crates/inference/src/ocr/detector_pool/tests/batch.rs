use image::RgbImage;
use oar_ocr::processors::{NormalizeImage, TensorLayout, types::ColorOrder};

use super::*;

/// A `width` × `height` frame padded below with black rows, its pixels from `paint`.
fn frame(width: u32, height: u32, paint: impl Fn(u32, u32) -> [u8; 3]) -> PaddedFrame {
    let padded_height = PaddedFrame::padded(height);
    let mut rgb = vec![0u8; width as usize * padded_height as usize * 3];
    for y in 0..height {
        for x in 0..width {
            let at = (y as usize * width as usize + x as usize) * 3;
            rgb[at..at + 3].copy_from_slice(&paint(x, y));
        }
    }
    PaddedFrame {
        width,
        height,
        padded_height,
        rgb,
    }
}

fn close(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() < 1e-4
}

#[test]
fn shapes_pad_height_and_width_to_multiples_of_32() {
    let shape = InputShape::for_frames(4, 1920, 1080);
    assert_eq!(shape.dims(), [4, 3, 1088, 1920]);
    assert_eq!(shape.len(), 4 * 3 * 1088 * 1920);
    assert_eq!(shape.output_len(), 4 * 1088 * 1920);
    assert_eq!(
        InputShape::for_frames(1, 1000, 700).dims(),
        [1, 3, 704, 1024]
    );
}

#[test]
fn a_pixel_is_normalised_in_bgr_planes_with_imagenet_statistics() {
    let shape = InputShape::for_frames(1, 3, 2);
    let frames = [frame(3, 2, |x, y| {
        if (x, y) == (1, 1) {
            [10, 200, 30]
        } else {
            [0, 0, 0]
        }
    })];
    let mut staging = vec![f32::NAN; shape.len()];
    normalize_into(&frames, shape, &mut staging).unwrap();
    let at = shape.width + 1;
    // Blue first: (30 / 255 − 0.485) / 0.229.
    assert!(close(staging[at], -1.604_17), "{}", staging[at]);
    // Green: (200 / 255 − 0.456) / 0.224.
    assert!(close(staging[shape.plane() + at], 1.465_69));
    // Red last: (10 / 255 − 0.406) / 0.225.
    assert!(close(staging[2 * shape.plane() + at], -1.630_17));
}

#[test]
fn padding_columns_rows_and_missing_frames_are_black() {
    let shape = InputShape::for_frames(3, 3, 2);
    let frames = [frame(3, 2, |_, _| [255, 255, 255])];
    let mut staging = vec![f32::NAN; shape.len()];
    normalize_into(&frames, shape, &mut staging).unwrap();
    let black = [-0.485 / 0.229, -0.456 / 0.224, -0.406 / 0.225];
    let plane = shape.plane();
    for channel in 0..3 {
        let first = &staging[channel * plane..(channel + 1) * plane];
        // Right of the frame's three columns, and below its two rows.
        assert!(close(first[5], black[channel]));
        assert!(close(first[10 * shape.width], black[channel]));
        assert!(!close(first[2], black[channel]));
    }
    for (index, value) in staging[3 * plane..].iter().enumerate() {
        let channel = (index / plane) % 3;
        assert!(close(*value, black[channel]), "slot value {index}");
    }
}

#[test]
fn values_match_oar_ocr_db_normalisation() {
    let paint = |x: u32, y: u32| {
        [
            (x * 7 + y) as u8,
            (x * 3 + y * 5) as u8,
            (255 - x - y) as u8,
        ]
    };
    let padded = frame(32, 20, paint);
    let image = RgbImage::from_raw(32, 32, padded.rgb.clone()).unwrap();
    let reference = NormalizeImage::with_color_order(
        Some(1.0 / 255.0),
        Some(vec![0.485, 0.456, 0.406]),
        Some(vec![0.229, 0.224, 0.225]),
        Some(TensorLayout::CHW),
        Some(ColorOrder::BGR),
    )
    .unwrap()
    .normalize_batch_refs(&[&image])
    .unwrap();
    let shape = InputShape::for_frames(1, 32, 20);
    let mut staging = vec![0.0; shape.len()];
    normalize_into(&[padded], shape, &mut staging).unwrap();
    let reference = reference.as_slice().unwrap();
    assert_eq!(reference.len(), staging.len());
    for (ours, theirs) in staging.iter().zip(reference) {
        assert!((ours - theirs).abs() < 1e-5, "{ours} against {theirs}");
    }
}

#[test]
fn frames_that_do_not_fit_the_shape_are_refused() {
    let shape = InputShape::for_frames(2, 8, 8);
    let good = frame(8, 8, |_, _| [1, 2, 3]);
    assert!(check_frames(std::slice::from_ref(&good), shape, 8, 8).is_ok());
    assert!(check_frames(&[], shape, 8, 8).is_err());
    let three = vec![good.clone(), good.clone(), good.clone()];
    assert!(check_frames(&three, shape, 8, 8).is_err());
    assert!(check_frames(&[frame(8, 6, |_, _| [0; 3])], shape, 8, 8).is_err());
    let mut short = good.clone();
    short.rgb.pop();
    assert!(check_frames(&[short], shape, 8, 8).is_err());
    let mut staging = vec![0.0; shape.len() - 1];
    assert!(normalize_into(&[good], shape, &mut staging).is_err());
}

#[test]
fn black_fill_covers_the_whole_batch() {
    let shape = InputShape::for_frames(2, 4, 4);
    let mut staging = vec![f32::NAN; shape.len()];
    fill_black(shape, &mut staging).unwrap();
    assert!(
        staging
            .iter()
            .all(|value| value.is_finite() && *value < 0.0)
    );
}
