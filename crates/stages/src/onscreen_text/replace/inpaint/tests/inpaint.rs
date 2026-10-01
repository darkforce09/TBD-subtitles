use super::*;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use image::{GrayImage, Luma, Rgb};
use job_model::onscreen::{PixelRect, ReplacedText};

const FILL: [u8; 3] = [250, 5, 250];

struct Temporary(PathBuf);

impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-inpaint-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("visual/masks")).unwrap();
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Fills masked pixels with `FILL` and counts its calls.
struct Constant {
    side: usize,
    calls: usize,
}

impl Inpaint for Constant {
    fn side(&self) -> usize {
        self.side
    }

    fn inpaint(&mut self, rgb: &[u8], mask: &[u8]) -> TextResult<Vec<u8>> {
        self.calls += 1;
        let mut out = rgb.to_vec();
        for (pixel, _) in mask.iter().enumerate().filter(|&(_, &m)| m > 0) {
            out[pixel * 3..][..3].copy_from_slice(&FILL);
        }
        Ok(out)
    }
}

fn model() -> Constant {
    Constant { side: 64, calls: 0 }
}

fn pattern(width: u32, height: u32, seed: u32) -> RgbImage {
    RgbImage::from_fn(width, height, |x, y| {
        Rgb([
            ((x * 7 + y * 3 + seed) % 256) as u8,
            ((x * 5 + y * 11) % 256) as u8,
            ((x * 13 + seed * 3) % 256) as u8,
        ])
    })
}

fn block(width: u32, height: u32) -> GrayImage {
    GrayImage::from_fn(width, height, |x, y| {
        let inside = x >= width / 4 && x < width / 2 && y >= height / 4 && y < height / 2;
        Luma([if inside { 255 } else { 0 }])
    })
}

/// Saves `source` and `mask` under `name` and returns a plate over them at `size`.
fn plate(root: &Path, name: &str, source: &RgbImage, mask: &GrayImage, size: (u32, u32)) -> Plate {
    let source_path = PathBuf::from(format!("visual/masks/{name}-source.png"));
    let mask_path = PathBuf::from(format!("visual/masks/{name}-mask.png"));
    source.save(root.join(&source_path)).unwrap();
    mask.save(root.join(&mask_path)).unwrap();
    Plate {
        first_frame: 0,
        last_frame: 9,
        rect: PixelRect {
            x: 10,
            y: 20,
            width: size.0,
            height: size.1,
        },
        shift: [0.0, 0.0],
        scale: 1.0,
        source: source_path,
        mask: mask_path,
        plate: None,
        patch: None,
        shifted: Vec::new(),
    }
}

fn text(id: &str, status: ReplaceStatus, plates: Vec<Plate>) -> ReplacedText {
    ReplacedText {
        id: id.into(),
        first_frame: 0,
        last_frame: 99,
        status,
        style: None,
        container: None,
        plates,
        preview: None,
        lettering_quad: None,
    }
}

fn document(texts: Vec<ReplacedText>) -> ReplacementDocument {
    ReplacementDocument {
        width: 1920,
        height: 1080,
        frame_count: 1000,
        texts,
    }
}

fn load(root: &Path, plate: &Plate) -> RgbImage {
    image::open(root.join(plate.plate.as_ref().unwrap()))
        .unwrap()
        .into_rgb8()
}

#[test]
fn pending_plates_are_filled_written_and_recorded() {
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let (source, mask) = (pattern(40, 30, 0), block(40, 30));
    let second = pattern(40, 30, 9);
    let plates = vec![
        plate(root, "a0", &source, &mask, (40, 30)),
        plate(root, "a1", &second, &mask, (40, 30)),
    ];
    let mut doc = document(vec![text("text:1/2", ReplaceStatus::Pending, plates)]);
    let mut model = model();
    let seen = Mutex::new(Vec::new());
    inpaint(&mut doc, root, &mut model, &|done, total| {
        seen.lock().unwrap().push((done, total));
    })
    .unwrap();
    assert_eq!(model.calls, 2);
    assert_eq!(*seen.lock().unwrap(), vec![(0, 2), (1, 2), (2, 2)]);
    let plates = &doc.texts[0].plates;
    assert_eq!(
        plates[0].plate.as_deref(),
        Some(Path::new("visual/plates/text_1_2/0.png"))
    );
    assert_eq!(
        plates[1].plate.as_deref(),
        Some(Path::new("visual/plates/text_1_2/1.png"))
    );
    for (plate, original) in plates.iter().zip([&source, &second]) {
        let out = load(root, plate);
        for (x, y, pixel) in out.enumerate_pixels() {
            if mask.get_pixel(x, y).0[0] > 0 {
                assert_eq!(pixel.0, FILL);
            } else {
                assert_eq!(pixel, original.get_pixel(x, y));
            }
        }
    }
    assert!(!root.join("visual/plates/text_1_2/0.png.partial").exists());
}

#[test]
fn every_size_regime_fills_the_mask_and_keeps_the_rest() {
    for size in [(60, 20), (100, 50), (300, 100)] {
        let dir = Temporary::new();
        let root = dir.0.as_path();
        let source = pattern(size.0, size.1, 1);
        let mask = block(size.0, size.1);
        let plates = vec![plate(root, "p", &source, &mask, size)];
        let mut doc = document(vec![text("t", ReplaceStatus::Pending, plates)]);
        inpaint(&mut doc, root, &mut model(), &|_, _| {}).unwrap();
        let out = load(root, &doc.texts[0].plates[0]);
        let near = fill::dilate(&mask);
        for (x, y, pixel) in out.enumerate_pixels() {
            if mask.get_pixel(x, y).0[0] > 0 {
                assert_eq!(pixel.0, FILL, "{size:?} ({x}, {y})");
            } else if near.get_pixel(x, y).0[0] == 0 {
                assert_eq!(pixel, source.get_pixel(x, y), "{size:?} ({x}, {y})");
            }
        }
    }
}

#[test]
fn an_empty_mask_costs_no_call_and_keeps_the_source() {
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let source = pattern(30, 20, 2);
    let empty = GrayImage::new(30, 20);
    let plates = vec![plate(root, "e", &source, &empty, (30, 20))];
    let mut doc = document(vec![text("t", ReplaceStatus::Pending, plates)]);
    let mut model = model();
    inpaint(&mut doc, root, &mut model, &|_, _| {}).unwrap();
    assert_eq!(model.calls, 0);
    assert_eq!(load(root, &doc.texts[0].plates[0]), source);
}

#[test]
fn identical_plates_are_filled_once() {
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let (source, mask) = (pattern(40, 30, 3), block(40, 30));
    let one = plate(root, "one", &source, &mask, (40, 30));
    let two = plate(root, "two", &source, &mask, (40, 30));
    let mut doc = document(vec![
        text("a", ReplaceStatus::Pending, vec![one]),
        text("b", ReplaceStatus::Pending, vec![two]),
    ]);
    let mut model = model();
    inpaint(&mut doc, root, &mut model, &|_, _| {}).unwrap();
    assert_eq!(model.calls, 1);
    let first = load(root, &doc.texts[0].plates[0]);
    assert_eq!(first, load(root, &doc.texts[1].plates[0]));
    assert_ne!(doc.texts[0].plates[0].plate, doc.texts[1].plates[0].plate);
}

#[test]
fn settled_occurrences_are_untouched() {
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let missing = Plate {
        source: "visual/masks/none.png".into(),
        mask: "visual/masks/none-mask.png".into(),
        ..plate(root, "x", &pattern(4, 4, 0), &block(4, 4), (4, 4))
    };
    let texts = vec![
        text("baked", ReplaceStatus::Baked, vec![missing.clone()]),
        text(
            "fallback",
            ReplaceStatus::Fallback("moving surface".into()),
            vec![missing],
        ),
    ];
    let mut doc = document(texts);
    let before = doc.clone();
    let mut model = model();
    inpaint(&mut doc, root, &mut model, &|_, _| {}).unwrap();
    assert_eq!(doc, before);
    assert_eq!(model.calls, 0);
}

#[test]
fn a_mis_sized_or_missing_file_fails() {
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let small_mask = plate(root, "s", &pattern(20, 10, 0), &block(10, 10), (20, 10));
    let mut doc = document(vec![text("t", ReplaceStatus::Pending, vec![small_mask])]);
    assert!(inpaint(&mut doc, root, &mut model(), &|_, _| {}).is_err());

    let wrong_rect = plate(root, "r", &pattern(20, 10, 0), &block(20, 10), (21, 10));
    let mut doc = document(vec![text("t", ReplaceStatus::Pending, vec![wrong_rect])]);
    assert!(inpaint(&mut doc, root, &mut model(), &|_, _| {}).is_err());

    let mut gone = plate(root, "g", &pattern(20, 10, 0), &block(20, 10), (20, 10));
    gone.mask = "visual/masks/absent.png".into();
    let mut doc = document(vec![text("t", ReplaceStatus::Pending, vec![gone])]);
    let error = inpaint(&mut doc, root, &mut model(), &|_, _| {}).unwrap_err();
    assert!(error.to_string().contains("absent.png"), "{error}");
}

#[test]
fn model_errors_fail_the_step() {
    struct Broken;
    impl Inpaint for Broken {
        fn side(&self) -> usize {
            64
        }
        fn inpaint(&mut self, _: &[u8], _: &[u8]) -> TextResult<Vec<u8>> {
            Err("out of memory".into())
        }
    }
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let plates = vec![plate(
        root,
        "b",
        &pattern(20, 10, 0),
        &block(20, 10),
        (20, 10),
    )];
    let mut doc = document(vec![text("t", ReplaceStatus::Pending, plates)]);
    let error = inpaint(&mut doc, root, &mut Broken, &|_, _| {}).unwrap_err();
    assert!(error.to_string().contains("out of memory"), "{error}");
}

#[test]
fn ids_become_distinct_folder_names() {
    let mut taken = HashSet::new();
    assert_eq!(unique_folder("scene/1:a", &mut taken), "scene_1_a");
    assert_eq!(unique_folder("scene_1_a", &mut taken), "scene_1_a-2");
    assert_eq!(unique_folder("scene?1?a", &mut taken), "scene_1_a-3");
    assert_eq!(unique_folder("", &mut taken), "_");
    assert_eq!(unique_folder("看板", &mut taken), "__");
}

#[test]
fn the_cache_drops_its_oldest_plates_beyond_its_bound() {
    let mut cache = PlateCache::default();
    for i in 0..(CACHE_ENTRIES as u32 + 6) {
        cache.insert(i.to_le_bytes().to_vec(), vec![1], RgbImage::new(2, 2));
    }
    assert_eq!(cache.entries.len(), CACHE_ENTRIES);
    assert!(cache.get(&0_u32.to_le_bytes(), &[1]).is_none());
    assert!(
        cache
            .get(&(CACHE_ENTRIES as u32).to_le_bytes(), &[1])
            .is_some()
    );
    assert!(
        cache
            .get(&(CACHE_ENTRIES as u32).to_le_bytes(), &[2])
            .is_none()
    );
    assert_eq!(cache.bytes, CACHE_ENTRIES * (4 + 1 + 12));
}

const WALL: [u8; 3] = [150, 150, 150];
const INK: [u8; 3] = [20, 20, 25];

/// Fills its first `leaves` calls by handing the picture back unchanged, strokes and all, and
/// every later call with the wall colour.
struct Stubborn {
    leaves: usize,
    calls: usize,
}

impl Inpaint for Stubborn {
    fn side(&self) -> usize {
        64
    }

    fn inpaint(&mut self, rgb: &[u8], mask: &[u8]) -> TextResult<Vec<u8>> {
        self.calls += 1;
        let mut out = rgb.to_vec();
        if self.calls > self.leaves {
            for (pixel, _) in mask.iter().enumerate().filter(|&(_, &m)| m > 0) {
                out[pixel * 3..][..3].copy_from_slice(&WALL);
            }
        }
        Ok(out)
    }
}

/// A wall with two thick dark strokes, the mask over them, and a text with that lettering style.
fn stroked_text(root: &Path) -> ReplacedText {
    let source = RgbImage::from_fn(60, 40, |x, y| {
        let stroke = (10..16).contains(&x) && (8..32).contains(&y)
            || (30..50).contains(&x) && (17..23).contains(&y);
        Rgb(if stroke { INK } else { WALL })
    });
    let mask = GrayImage::from_fn(60, 40, |x, y| {
        let near = (8..18).contains(&x) && (6..34).contains(&y)
            || (28..52).contains(&x) && (15..25).contains(&y);
        Luma([if near { 255 } else { 0 }])
    });
    let mut text = text(
        "wall",
        ReplaceStatus::Pending,
        vec![plate(root, "wall", &source, &mask, (60, 40))],
    );
    text.style = Some(job_model::onscreen::LetteringStyle {
        fill_rgb: INK,
        outline_rgb: None,
        outline_px: 0.0,
        soft_outline: false,
        stroke_px: 6.0,
        line_height_px: 30.0,
    });
    text
}

#[test]
fn a_clean_fill_passes_the_residue_check_at_once() {
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let mut doc = document(vec![stroked_text(root)]);
    let mut model = Stubborn {
        leaves: 0,
        calls: 0,
    };
    inpaint(&mut doc, root, &mut model, &|_, _| {}).unwrap();
    assert_eq!(model.calls, 1);
    let text = &doc.texts[0];
    assert_eq!(text.status, ReplaceStatus::Pending);
    assert_eq!(
        text.plates[0].mask,
        PathBuf::from("visual/masks/wall-mask.png")
    );
    assert!(load(root, &text.plates[0]).pixels().all(|p| p.0 == WALL));
}

#[test]
fn strokes_the_fill_leaves_are_filled_again_under_a_wider_mask() {
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let mut doc = document(vec![stroked_text(root)]);
    let mut model = Stubborn {
        leaves: 1,
        calls: 0,
    };
    inpaint(&mut doc, root, &mut model, &|_, _| {}).unwrap();
    assert_eq!(model.calls, 2);
    let text = &doc.texts[0];
    assert_eq!(text.status, ReplaceStatus::Pending);
    let widened = PathBuf::from("visual/plates/wall/0-mask.png");
    assert_eq!(text.plates[0].mask, widened);
    let original = image::open(root.join("visual/masks/wall-mask.png"))
        .unwrap()
        .into_luma8();
    let wider = image::open(root.join(&widened)).unwrap().into_luma8();
    let count = |m: &GrayImage| m.pixels().filter(|p| p.0[0] > 0).count();
    assert!(count(&wider) > count(&original));
    assert!(
        original
            .pixels()
            .zip(wider.pixels())
            .all(|(o, w)| o.0[0] == 0 || w.0[0] > 0)
    );
    assert!(load(root, &text.plates[0]).pixels().all(|p| p.0 == WALL));
}

#[test]
fn strokes_that_survive_the_wider_fill_are_left_to_the_read_back_check() {
    let dir = Temporary::new();
    let root = dir.0.as_path();
    let mut doc = document(vec![stroked_text(root)]);
    let mut model = Stubborn {
        leaves: 2,
        calls: 0,
    };
    let seen = Mutex::new(Vec::new());
    inpaint(&mut doc, root, &mut model, &|done, total| {
        seen.lock().unwrap().push((done, total));
    })
    .unwrap();
    assert_eq!(model.calls, 2);
    assert_eq!(*seen.lock().unwrap(), vec![(0, 1), (1, 1)]);
    let text = &doc.texts[0];
    assert_eq!(text.status, ReplaceStatus::Pending);
    assert_eq!(
        text.plates[0].plate,
        Some(PathBuf::from("visual/plates/wall/0.png"))
    );
    assert_eq!(
        text.plates[0].mask,
        PathBuf::from("visual/plates/wall/0-mask.png")
    );
    assert!(root.join("visual/plates/wall/0.png").is_file());
}
