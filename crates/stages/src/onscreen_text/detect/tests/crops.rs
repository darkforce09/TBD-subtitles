use super::*;
use image::Rgb;
use job_model::onscreen::{TextFrame, TextPresentation, TextProvenance};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Temporary(PathBuf);

impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-detect-crops-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(path.join("visual/crops")).unwrap();
        std::fs::create_dir_all(path.join("visual/keyframes")).unwrap();
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

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

#[test]
fn keyframe_stills_are_saved_at_most_1280_pixels_wide_keeping_the_aspect() {
    let root = Temporary::new();
    let wide = root.0.join("wide.png");
    save_keyframe(&RgbImage::new(1920, 1080), &wide).unwrap();
    assert_eq!(image::image_dimensions(&wide).unwrap(), (1280, 720));
    let small = root.0.join("small.png");
    save_keyframe(&RgbImage::new(640, 360), &small).unwrap();
    assert_eq!(image::image_dimensions(&small).unwrap(), (640, 360));
}

#[test]
fn the_keyframe_takes_the_best_full_resolution_region_and_one_surface_for_all_frames() {
    let root = Temporary::new();
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
    let proxy = rectangle(57.0, 39.0, 144.0, 42.0);
    let mut item = occurrence(&[proxy, proxy, proxy]);
    let exact = rectangle(60.0, 40.0, 140.0, 40.0);
    let found = [(rectangle(0.0, 0.0, 30.0, 20.0), 0.9), (exact, 0.8)];
    let image = Path::new("visual/keyframes/frame-00000001.png");
    assert!(confirm(&mut item, 1, &still, &found, image, &root.0).unwrap());
    assert_eq!(item.frames[1].quad, exact);
    assert_eq!(
        item.frames[0].quad, proxy,
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
    let saved = image::open(root.0.join(&item.crops[0])).unwrap();
    assert_eq!((saved.width(), saved.height()), (140, 40));
}

#[test]
fn an_unconfirmed_keyframe_leaves_the_occurrence_untouched_and_unconfirmed() {
    let root = Temporary::new();
    let still = RgbImage::from_pixel(320, 180, Rgb([90, 110, 130]));
    let proxy = rectangle(57.0, 39.0, 144.0, 42.0);
    let mut item = occurrence(&[proxy, proxy]);
    let elsewhere = [(rectangle(250.0, 120.0, 60.0, 40.0), 0.9)];
    let image = Path::new("visual/keyframes/frame-00000000.png");
    assert!(!confirm(&mut item, 0, &still, &elsewhere, image, &root.0).unwrap());
    assert_eq!(item.frames[0].quad, proxy);
    assert!(item.warnings.is_empty());
    assert!(item.keyframe.is_none());
    assert!(!root.0.join("visual/crops/text-000001.png").exists());
    assert!(confirm(&mut item, 5, &still, &elsewhere, image, &root.0).is_err());
}
