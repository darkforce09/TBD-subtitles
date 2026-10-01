use super::*;
use image::Rgb;
use job_model::onscreen::{TextFrame, TextPresentation, TextProvenance};

use crate::onscreen_text::detect::regions::same_signature;

fn rectangle(left: f64, top: f64, width: f64, height: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point {
            x: left + width,
            y: top,
        },
        Point {
            x: left + width,
            y: top + height,
        },
        Point {
            x: left,
            y: top + height,
        },
    ])
}

fn lettering(width: u32, height: u32) -> RgbImage {
    let mut image = RgbImage::from_pixel(width, height, Rgb([240; 3]));
    for left in (16..width.saturating_sub(20)).step_by(24) {
        for y in 8..height - 8 {
            for x in left..left + 4 {
                image.put_pixel(x, y, Rgb([20; 3]));
            }
        }
        for y in 8..12 {
            for x in left..left + 13 {
                image.put_pixel(x, y, Rgb([20; 3]));
            }
        }
    }
    image
}

fn occurrence(quads: &[Quad]) -> TextOccurrence {
    TextOccurrence {
        id: "text-000001".into(),
        start_s: 0.0,
        end_s: quads.len() as f64,
        japanese: String::new(),
        english: None,
        confidence: 0.9,
        crops: vec![PathBuf::from("visual/crops/text-000001.png")],
        frames: quads
            .iter()
            .enumerate()
            .map(|(i, &quad)| TextFrame {
                time_s: i as f64,
                end_s: i as f64 + 1.0,
                quad,
                confidence: 0.9,
                surface_rgb: None,
            })
            .collect(),
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: None,
        ruby: Vec::new(),
    }
}

#[test]
fn texture_on_the_border_rejects_the_surface() {
    let mut image = RgbImage::from_pixel(100, 40, Rgb([240; 3]));
    assert_eq!(simple_surface(&image), Some([240; 3]));
    image.put_pixel(0, 20, Rgb([20; 3]));
    assert_eq!(simple_surface(&image), None);
}

#[test]
fn perspective_rectification_recovers_the_lettering_plane() {
    let original = lettering(192, 48);
    let plane = rectangle(0.0, 0.0, 191.0, 47.0);
    let sign = Quad([
        Point { x: 40.0, y: 20.0 },
        Point { x: 235.0, y: 65.0 },
        Point { x: 216.0, y: 124.0 },
        Point { x: 30.0, y: 82.0 },
    ]);
    let transform = geometry::quad_to_quad(plane, sign).unwrap();
    let projection =
        Projection::from_matrix(std::array::from_fn(|i| transform[(i / 3, i % 3)] as f32)).unwrap();
    let mut scene = RgbImage::from_pixel(280, 150, Rgb([240; 3]));
    warp_into(
        &original,
        &projection,
        Interpolation::Bilinear,
        Rgb([240; 3]),
        &mut scene,
    );
    // This pixel lies outside the sign, but on its raw axis-aligned crop border.
    scene.put_pixel(30, 20, Rgb([0; 3]));
    let restored = imageops::resize(&crop(&scene, sign), 192, 48, imageops::FilterType::Triangle);
    let error = original
        .as_raw()
        .iter()
        .zip(restored.as_raw())
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum::<u64>() as f64
        / original.as_raw().len() as f64;
    assert!(error < 15.0, "rectified average error {error}");
    let raw = imageops::resize(
        &axis_crop(&scene, sign),
        192,
        48,
        imageops::FilterType::Triangle,
    );
    let raw_error = original
        .as_raw()
        .iter()
        .zip(raw.as_raw())
        .map(|(a, b)| u64::from(a.abs_diff(*b)))
        .sum::<u64>() as f64
        / original.as_raw().len() as f64;
    assert!(
        raw_error > error * 2.0,
        "raw {raw_error}, rectified {error}"
    );
    assert_eq!(simple_surface(&axis_crop(&scene, sign)), None);
}

/// A full-range yuv420p picture with neutral chroma whose luma is `image`'s red channel.
fn grey_picture(image: &RgbImage) -> Vec<u8> {
    let mut bytes: Vec<u8> = image.pixels().map(|pixel| pixel.0[0]).collect();
    bytes.resize(bytes.len() * 3 / 2, 128);
    bytes
}

fn full_range() -> Coefficients {
    Coefficients::new(media_io::yuv::Matrix::Bt709, media_io::yuv::Range::Full)
}

#[test]
fn a_grey_crop_from_luma_matches_the_colour_crop_axis_aligned_and_rectified() {
    let original = lettering(192, 48);
    let plane = rectangle(0.0, 0.0, 191.0, 47.0);
    let sign = Quad([
        Point { x: 40.0, y: 20.0 },
        Point { x: 235.0, y: 65.0 },
        Point { x: 216.0, y: 124.0 },
        Point { x: 30.0, y: 82.0 },
    ]);
    let transform = geometry::quad_to_quad(plane, sign).unwrap();
    let projection =
        Projection::from_matrix(std::array::from_fn(|i| transform[(i / 3, i % 3)] as f32)).unwrap();
    let mut scene = RgbImage::from_pixel(280, 150, Rgb([240; 3]));
    warp_into(
        &original,
        &projection,
        Interpolation::Bilinear,
        Rgb([240; 3]),
        &mut scene,
    );
    let bytes = grey_picture(&scene);
    let picture = Yuv420::planar(&bytes, 280, 150).unwrap();
    for quad in [sign, rectangle(30.0, 20.0, 120.0, 40.0)] {
        let grey = grey_crop(&picture, &full_range(), quad);
        let colour = imageops::grayscale(&crop(&scene, quad));
        assert_eq!(grey.dimensions(), colour.dimensions());
        let error = grey
            .as_raw()
            .iter()
            .zip(colour.as_raw())
            .map(|(a, b)| u64::from(a.abs_diff(*b)))
            .sum::<u64>() as f64
            / grey.as_raw().len() as f64;
        assert!(error < 1.0, "grey and colour crops differ by {error}");
    }
    let edge = rectangle(-5.0, 140.0, 20.0, 30.0);
    let clamped = grey_crop(&picture, &full_range(), edge);
    assert_eq!(clamped.dimensions(), (15, 10), "clamped to the picture");
}

#[test]
fn a_signature_reads_the_anchor_box_from_luma() {
    let scene = lettering(192, 48);
    let bytes = grey_picture(&scene);
    let picture = Yuv420::planar(&bytes, 192, 48).unwrap();
    let anchor = rectangle(10.0, 4.0, 150.0, 40.0);
    let from_luma = picture_at(&picture, &full_range(), anchor);
    let from_colour = signature(&imageops::grayscale(&axis_crop(&scene, anchor)));
    assert!(same_signature(&from_colour, &from_luma));
    assert_eq!(from_colour, from_luma);
}

#[test]
fn the_keyframe_takes_the_best_full_resolution_region_and_one_surface_for_all_frames() {
    let mut still = RgbImage::from_pixel(320, 180, Rgb([90, 110, 130]));
    for y in 40..80 {
        for x in 60..200 {
            still.put_pixel(x, y, Rgb([240; 3]));
        }
    }
    for y in 55..65 {
        for x in 100..160 {
            still.put_pixel(x, y, Rgb([20; 3]));
        }
    }
    let screened = rectangle(57.0, 39.0, 144.0, 42.0);
    let mut item = occurrence(&[screened, screened, screened]);
    let exact = rectangle(60.0, 40.0, 140.0, 40.0);
    let found = [(rectangle(0.0, 0.0, 30.0, 20.0), 0.9), (exact, 0.8)];
    let image = Path::new("visual/keyframes/frame-00000001.png");
    let (path, rectified) = confirm(&mut item, 1, &still, &found, image)
        .unwrap()
        .unwrap();
    assert_eq!(item.frames[1].quad, exact);
    assert_eq!(
        item.frames[0].quad, screened,
        "only the keyframe frame changes"
    );
    assert!(item.frames.iter().all(|f| f.surface_rgb == Some([240; 3])));
    assert!(item.warnings.is_empty());
    assert_eq!(
        item.keyframe,
        Some(TextKeyframe {
            time_s: 1.0,
            image: image.to_path_buf()
        })
    );
    assert_eq!(item.crops, [PathBuf::from("visual/crops/text-000001.png")]);
    assert_eq!(path, item.crops[0]);
    assert_eq!(rectified.dimensions(), (140, 40));
}

#[test]
fn an_unconfirmed_keyframe_leaves_the_occurrence_untouched_and_unconfirmed() {
    let still = RgbImage::from_pixel(320, 180, Rgb([90, 110, 130]));
    let screened = rectangle(57.0, 39.0, 144.0, 42.0);
    let mut item = occurrence(&[screened, screened]);
    let elsewhere = [(rectangle(250.0, 120.0, 60.0, 40.0), 0.9)];
    let image = Path::new("visual/keyframes/frame-00000000.png");
    assert!(
        confirm(&mut item, 0, &still, &elsewhere, image)
            .unwrap()
            .is_none()
    );
    assert_eq!(item.frames[0].quad, screened);
    assert!(item.warnings.is_empty());
    assert!(item.keyframe.is_none());
    assert!(confirm(&mut item, 5, &still, &elsewhere, image).is_err());
}
