use std::path::PathBuf;

use image::{Rgba, RgbaImage};
use job_model::onscreen::{
    PixelRect, Plate, ReplaceStatus, ReplacedText, ReplacementDocument, ShiftedPatch,
};

use super::*;
use crate::localize::blend::ChromaBlocks;
use crate::localize::colour::{Conversion, Matrix, Range};
use crate::localize::motion::Motion;

fn rect(x: u32, width: u32) -> PixelRect {
    PixelRect {
        x,
        y: 0,
        width,
        height: 2,
    }
}

fn plate(first: u64, last: u64, name: Option<&str>) -> Plate {
    Plate {
        first_frame: first,
        last_frame: last,
        rect: rect(0, 2),
        shift: [0.0, 0.0],
        scale: 1.0,
        source: PathBuf::from("source.png"),
        mask: PathBuf::from("mask.png"),
        plate: None,
        patch: name.map(PathBuf::from),
        shifted: Vec::new(),
    }
}

fn text(id: &str, status: ReplaceStatus, plates: Vec<Plate>) -> ReplacedText {
    ReplacedText {
        id: id.into(),
        first_frame: plates.first().map_or(0, |p| p.first_frame),
        last_frame: plates.last().map_or(0, |p| p.last_frame),
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
        width: 64,
        height: 64,
        frame_count: 100,
        texts,
    }
}

fn names(schedule: &Schedule) -> Vec<String> {
    schedule
        .active()
        .iter()
        .map(|patch| patch.path.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn patches_are_active_exactly_on_their_frames_in_document_order() {
    let doc = document(vec![
        text(
            "T1",
            ReplaceStatus::Baked,
            vec![plate(5, 9, Some("a1")), plate(10, 12, Some("a2"))],
        ),
        text("T2", ReplaceStatus::Baked, vec![plate(2, 10, Some("b"))]),
    ]);
    let mut schedule = Schedule::new(&doc, &Motion::default()).unwrap();
    assert!(!schedule.is_empty());
    let mut seen = Vec::new();
    let mut ended = Vec::new();
    for frame in 0..14 {
        ended.push(schedule.advance(frame).unwrap());
        seen.push(names(&schedule));
    }
    assert!(seen[1].is_empty());
    assert_eq!(seen[2], ["b"]);
    assert_eq!(seen[4], ["b"]);
    // T1 comes first in the document although T2 started earlier.
    assert_eq!(seen[5], ["a1", "b"]);
    assert_eq!(seen[9], ["a1", "b"]);
    assert_eq!(seen[10], ["a2", "b"]);
    assert_eq!(seen[11], ["a2"]);
    assert_eq!(seen[12], ["a2"]);
    assert!(seen[13].is_empty());
    assert_eq!(ended[10], [0]);
    assert_eq!(ended[11], [2]);
    assert_eq!(ended[13], [1]);
}

#[test]
fn the_changed_spans_are_every_frame_a_patch_covers_merged_in_order() {
    let doc = document(vec![
        text(
            "T1",
            ReplaceStatus::Baked,
            vec![plate(40, 49, Some("a1")), plate(50, 55, Some("a2"))],
        ),
        text("T2", ReplaceStatus::Baked, vec![plate(5, 9, Some("b"))]),
        text("T3", ReplaceStatus::Baked, vec![plate(8, 12, Some("c"))]),
        text("T4", ReplaceStatus::Baked, vec![plate(14, 20, Some("d"))]),
        text("T5", ReplaceStatus::Baked, vec![plate(60, 70, None)]),
        text(
            "T6",
            ReplaceStatus::Fallback("too busy".into()),
            vec![plate(80, 90, Some("f"))],
        ),
    ]);
    let mut schedule = Schedule::new(&doc, &Motion::default()).unwrap();
    // Overlapping and touching plates join; a gap of one frame keeps runs apart; plates without
    // a patch and occurrences left in Japanese change nothing.
    assert_eq!(schedule.changed_spans(), vec![(5, 12), (14, 20), (40, 55)]);
    schedule.advance(16).unwrap();
    assert_eq!(
        schedule.changed_spans(),
        vec![(17, 20), (40, 55)],
        "frames already advanced to are no longer due"
    );
    assert!(
        Schedule::new(&ReplacementDocument::default(), &Motion::default())
            .unwrap()
            .changed_spans()
            .is_empty()
    );
}

#[test]
fn only_baked_occurrences_with_composed_patches_are_scheduled() {
    let doc = document(vec![
        text("T1", ReplaceStatus::Pending, vec![plate(0, 5, Some("p"))]),
        text(
            "T2",
            ReplaceStatus::Fallback("too busy".into()),
            vec![plate(0, 5, Some("f"))],
        ),
        text("T3", ReplaceStatus::Baked, vec![plate(0, 5, None)]),
    ]);
    assert!(Schedule::new(&doc, &Motion::default()).unwrap().is_empty());
    assert!(
        Schedule::new(&ReplacementDocument::default(), &Motion::default())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn skipped_frames_still_activate_and_end_patches() {
    let doc = document(vec![
        text("T1", ReplaceStatus::Baked, vec![plate(3, 4, Some("short"))]),
        text("T2", ReplaceStatus::Baked, vec![plate(3, 20, Some("long"))]),
    ]);
    let mut schedule = Schedule::new(&doc, &Motion::default()).unwrap();
    schedule.advance(0).unwrap();
    // Jumping past the short patch's span never activates it.
    schedule.advance(6).unwrap();
    assert_eq!(names(&schedule), ["long"]);
}

#[test]
fn a_plate_whose_frames_move_takes_the_patch_of_each_frame_s_shift() {
    let mut moved = plate(10, 15, Some("own"));
    moved.shifted = vec![ShiftedPatch {
        shift: [1.0, 0.0],
        patch: PathBuf::from("right"),
    }];
    let doc = document(vec![text("T1", ReplaceStatus::Baked, vec![moved])]);
    let mut motion = Motion::default();
    for (frame, dx) in [(10, 0.0), (11, 1.0), (12, 1.0), (13, 0.0), (14, 1.0)] {
        motion.add("T1", frame, 0, [dx, 0.0]);
    }
    let mut schedule = Schedule::new(&doc, &motion).unwrap();
    let mut seen = Vec::new();
    let mut ended = Vec::new();
    for frame in 10..17 {
        ended.push(schedule.advance(frame).unwrap());
        seen.push(names(&schedule));
    }
    // Frame 15 has no row and takes the plate's own patch.
    assert_eq!(
        seen,
        [
            vec!["own"],
            vec!["right"],
            vec!["right"],
            vec!["own"],
            vec!["right"],
            vec!["own"],
            vec![],
        ]
    );
    // Each file leaves the cache only after the last frame that blends it.
    assert_eq!(ended[5], [1]);
    assert_eq!(ended[6], [0]);
    assert!(ended[..5].iter().all(Vec::is_empty));
}

#[test]
fn a_shift_without_its_patch_is_an_error() {
    let doc = document(vec![text(
        "T1",
        ReplaceStatus::Baked,
        vec![plate(0, 3, Some("own"))],
    )]);
    let mut motion = Motion::default();
    motion.add("T1", 1, 0, [2.0, 0.0]);
    let error = Schedule::new(&doc, &motion).unwrap_err();
    assert!(error.0.contains("no patch for the shift"), "{error}");
}

#[test]
fn frames_must_advance() {
    let mut schedule = Schedule::new(&document(Vec::new()), &Motion::default()).unwrap();
    schedule.advance(4).unwrap();
    assert!(schedule.advance(4).is_err());
    assert!(schedule.advance(3).is_err());
    assert!(schedule.advance(5).is_ok());
}

/// A converted patch of `bytes` bytes: luma and alpha only.
fn sized(bytes: usize) -> FramePatch {
    let pixels = bytes / 3;
    FramePatch {
        rect: rect(0, 2),
        luma: vec![0; pixels],
        alpha: vec![0; pixels],
        chroma: ChromaBlocks {
            x: 0,
            y: 0,
            columns: 0,
            rows: 0,
            cb: Vec::new(),
            cr: Vec::new(),
            coverage: Vec::new(),
        },
    }
}

fn entry(order: usize) -> ScheduledPatch {
    ScheduledPatch {
        order,
        file: order,
        first_frame: 0,
        last_frame: 10,
        rect: rect(0, 2),
        path: PathBuf::from(format!("{order}.png")),
    }
}

#[test]
fn the_cache_evicts_the_least_recently_used_patch_to_stay_in_budget() {
    let mut cache = PatchCache::new(300);
    let loads = std::cell::Cell::new(0);
    let get = |cache: &mut PatchCache, order: usize| {
        cache
            .get(&entry(order), |_| {
                loads.set(loads.get() + 1);
                Ok(sized(120))
            })
            .map(|patch| patch.bytes())
            .unwrap()
    };
    assert_eq!(get(&mut cache, 0), 120);
    get(&mut cache, 1);
    assert_eq!(cache.used(), 240);
    // Using 0 again makes 1 the oldest, which the third patch pushes out.
    get(&mut cache, 0);
    get(&mut cache, 2);
    assert_eq!(cache.len(), 2);
    assert!(cache.used() <= 300);
    get(&mut cache, 0);
    assert_eq!(loads.get(), 3);
    get(&mut cache, 1);
    assert_eq!(loads.get(), 4);
    assert!(cache.used() <= 300);
    cache.remove(1);
    cache.remove(7);
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.used(), 120);
}

#[test]
fn a_patch_larger_than_the_budget_is_still_held_alone() {
    let mut cache = PatchCache::new(100);
    cache.get(&entry(0), |_| Ok(sized(60))).unwrap();
    cache.get(&entry(1), |_| Ok(sized(150))).unwrap();
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.used(), 150);
    cache.remove(1);
    assert!(cache.is_empty());
    assert_eq!(cache.used(), 0);
}

#[test]
fn a_failed_load_leaves_the_cache_unchanged() {
    let mut cache = PatchCache::new(100);
    assert!(cache.get(&entry(0), |_| Err("unreadable".into())).is_err());
    assert!(cache.is_empty());
}

#[test]
fn loading_reads_the_png_and_refuses_one_of_the_wrong_size() {
    let root = std::env::temp_dir().join(format!("tbd-localize-load-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]))
        .save(root.join("good.png"))
        .unwrap();
    RgbaImage::from_pixel(3, 2, Rgba([255, 255, 255, 255]))
        .save(root.join("wide.png"))
        .unwrap();
    let conversion = Conversion {
        matrix: Matrix::Bt709,
        range: Range::Limited,
        bits: 8,
    };
    let mut good = entry(0);
    good.path = PathBuf::from("good.png");
    let patch = load(&root, &good, conversion).unwrap();
    assert_eq!(patch.luma, vec![235; 4]);
    let mut wide = entry(1);
    wide.path = PathBuf::from("wide.png");
    let error = load(&root, &wide, conversion).unwrap_err();
    assert!(error.0.contains("wide.png"), "{error}");
    assert!(error.0.contains("3x2"), "{error}");
    let mut missing = entry(2);
    missing.path = PathBuf::from("missing.png");
    assert!(load(&root, &missing, conversion).is_err());
    std::fs::remove_dir_all(&root).unwrap();
}
