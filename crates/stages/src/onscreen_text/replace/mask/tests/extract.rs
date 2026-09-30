use std::path::Path;

use image::GrayImage;
use job_model::onscreen::{
    Quad, ReplaceStatus, ReplacementDocument, TextDocument, TextOccurrence, TextTreatment,
};

use super::cluster::{distance, lab};
use super::fixtures::{
    Background, FILL, FPS, Glyphs, OUTLINE, Scripted, hash, iou, job_dir, occurrence,
    still_occurrence,
};
use super::{NEARBY, NO_FRAMES, extract};

const SIZE: (u32, u32) = (640, 360);

fn document(occurrences: Vec<TextOccurrence>) -> TextDocument {
    TextDocument {
        width: SIZE.0,
        height: SIZE.1,
        occurrences,
        ..TextDocument::default()
    }
}

fn run(text: &TextDocument, source: &mut Scripted, root: &Path) -> ReplacementDocument {
    let document = extract(text, source, root, &|_, _| {}).expect("extraction succeeds");
    document.validate().expect("the document validates");
    document
}

fn read_mask(root: &Path, path: &Path) -> GrayImage {
    image::open(root.join(path)).expect("mask PNG").to_luma8()
}

fn delta_e(a: [u8; 3], b: [u8; 3]) -> f32 {
    distance(lab(a), lab(b))
}

#[test]
fn masks_and_styles_match_the_drawn_writing_on_every_background() {
    for background in [
        Background::Flat,
        Background::Gradient,
        Background::Checker,
        Background::Noise,
    ] {
        for outline in [0, 2] {
            let name = format!("style-{background:?}-{outline}");
            let root = job_dir(&name);
            let glyphs = Glyphs::row(200, 160, outline);
            let paint = glyphs.clone();
            let mut source = Scripted::new(3, SIZE, move |_, x, y| {
                paint.paint(x, y, background.at(x, y))
            });
            let text = document(vec![still_occurrence("t1", glyphs.quad(4.0), 0, 2)]);
            let result = run(&text, &mut source, &root);
            let item = &result.texts[0];
            assert_eq!(item.status, ReplaceStatus::Pending, "{name}");
            assert_eq!(item.plates.len(), 1, "{name}");
            let plate = &item.plates[0];
            let mask = read_mask(&root, &plate.mask);
            assert_eq!(mask.dimensions(), (plate.rect.width, plate.rect.height));
            let score = iou(&mask, &glyphs.truth(plate.rect, 2));
            assert!(score >= 0.85, "{name}: mask IoU {score}");
            let style = item.style.as_ref().expect("a style");
            assert!(
                delta_e(style.fill_rgb, FILL) <= 10.0,
                "{name}: fill {:?}",
                style.fill_rgb
            );
            if outline > 0 {
                let measured = style.outline_rgb.expect("an outline");
                assert!(
                    delta_e(measured, OUTLINE) <= 10.0,
                    "{name}: outline {measured:?}"
                );
                assert!(
                    (style.outline_px - 2.0).abs() < 1.5,
                    "{name}: {}",
                    style.outline_px
                );
            } else {
                assert_eq!(style.outline_rgb, None, "{name}");
                assert_eq!(style.outline_px, 0.0);
            }
            assert!(
                (style.stroke_px - 5.0).abs() < 2.0,
                "{name}: stroke {}",
                style.stroke_px
            );
            assert!(!style.soft_outline, "{name}");
            assert_eq!(style.line_height_px, 38.0 + 2.0 * outline as f64);
        }
    }
}

#[test]
fn pure_noise_is_not_separated() {
    let root = job_dir("noise");
    let quads = [
        Glyphs::row(200, 160, 0).quad(4.0),
        super::fixtures::rect_quad(100.0, 100.0, 140.0, 124.0),
        super::fixtures::rect_quad(50.0, 40.0, 450.0, 300.0),
    ];
    for seed in 0..6u64 {
        let colour = seed % 2 == 0;
        let mut source = Scripted::new(2, SIZE, move |_, x, y| {
            let h = hash(u64::from(x), u64::from(y), seed);
            if colour {
                [h as u8, (h >> 8) as u8, (h >> 16) as u8]
            } else {
                [h as u8; 3]
            }
        });
        let occurrences = quads
            .iter()
            .enumerate()
            .map(|(i, q)| still_occurrence(&format!("n{i}"), *q, 0, 1))
            .collect();
        let result = run(&document(occurrences), &mut source, &root);
        for text in &result.texts {
            assert_eq!(
                text.status,
                ReplaceStatus::Fallback(super::segment::UNSEPARATED.into()),
                "seed {seed}, {}",
                text.id
            );
            assert!(text.plates.is_empty());
        }
    }
    assert!(!root.join("visual/masks/n0").exists());
}

#[test]
fn nearby_placement_chosen_in_review_falls_back_without_decoding() {
    let root = job_dir("nearby");
    let glyphs = Glyphs::row(200, 160, 0);
    let mut item = still_occurrence("near", glyphs.quad(4.0), 0, 2);
    item.presentation.treatment = TextTreatment::Nearby;
    item.reviewed = true;
    let mut source = Scripted::new(3, SIZE, |_, _, _| [0, 0, 0]);
    let result = run(&document(vec![item]), &mut source, &root);
    assert_eq!(
        result.texts[0].status,
        ReplaceStatus::Fallback(NEARBY.into())
    );
    assert!(result.texts[0].plates.is_empty() && result.texts[0].style.is_none());
    assert_eq!(
        (result.texts[0].first_frame, result.texts[0].last_frame),
        (0, 2)
    );
    assert!(source.requests.is_empty());
}

#[test]
fn nearby_placement_from_earlier_steps_still_tries_replacement() {
    let root = job_dir("nearby-auto");
    let glyphs = Glyphs::row(200, 160, 0);
    let mut item = still_occurrence("moving", glyphs.quad(4.0), 0, 2);
    item.presentation.treatment = TextTreatment::Nearby;
    let mut source = Scripted::new(3, SIZE, |_, _, _| [0, 0, 0]);
    let result = run(&document(vec![item]), &mut source, &root);
    assert_ne!(
        result.texts[0].status,
        ReplaceStatus::Fallback(NEARBY.into())
    );
    assert!(!source.requests.is_empty());
}

#[test]
fn only_displayable_translated_occurrences_are_listed_in_order() {
    let root = job_dir("candidates");
    let quad = Glyphs::row(200, 160, 0).quad(4.0);
    let base = still_occurrence("keep-1", quad, 0, 1);
    let mut untranslated = base.clone();
    untranslated.id = "untranslated".into();
    untranslated.english = None;
    let mut blank = base.clone();
    blank.id = "blank".into();
    blank.english = Some("  ".into());
    let mut doubtful = base.clone();
    doubtful.id = "doubtful".into();
    doubtful.confidence = 0.5;
    let mut reviewed = doubtful.clone();
    reviewed.id = "keep-2".into();
    reviewed.reviewed = true;
    reviewed.presentation.treatment = TextTreatment::Nearby;
    let mut frameless = base.clone();
    frameless.id = "frameless".into();
    frameless.frames.clear();
    let mut keyless = base.clone();
    keyless.id = "keyless".into();
    keyless.keyframe = None;
    let mut nan = base.clone();
    nan.id = "nan".into();
    nan.confidence = f64::NAN;
    let mut first = base.clone();
    first.presentation.treatment = TextTreatment::Nearby;
    let text = document(vec![
        untranslated,
        first,
        blank,
        doubtful,
        reviewed,
        frameless,
        keyless,
        nan,
    ]);
    let mut source = Scripted::new(2, SIZE, |_, _, _| [0, 0, 0]);
    let calls = std::sync::atomic::AtomicUsize::new(0);
    let result = extract(&text, &mut source, &root, &|done, total| {
        assert_eq!(total, 2);
        assert_eq!(
            done,
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1
        );
    })
    .expect("extraction succeeds");
    let ids: Vec<&str> = result.texts.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["keep-1", "keep-2"]);
    assert_eq!(calls.into_inner(), 2);
    assert_eq!(
        (result.width, result.height, result.frame_count),
        (640, 360, 2)
    );
}

#[test]
fn writing_between_two_frame_starts_falls_back() {
    let root = job_dir("between");
    let quad = Glyphs::row(200, 160, 0).quad(4.0);
    let start = 1.0 / FPS + 0.01;
    let item = occurrence("gap", &[(start, start + 0.01, quad)], start);
    let mut source = Scripted::new(4, SIZE, |_, _, _| [0, 0, 0]);
    let result = run(&document(vec![item]), &mut source, &root);
    assert_eq!(
        result.texts[0].status,
        ReplaceStatus::Fallback(NO_FRAMES.into())
    );
    assert_eq!(
        (result.texts[0].first_frame, result.texts[0].last_frame),
        (1, 1)
    );
}

#[test]
fn a_background_change_starts_a_new_plate() {
    let root = job_dir("change");
    let glyphs = Glyphs::row(200, 160, 2);
    let paint = glyphs.clone();
    let mut source = Scripted::new(10, SIZE, move |index, x, y| {
        let background = if index < 5 {
            [60, 90, 150]
        } else {
            [90, 60, 70]
        };
        paint.paint(x, y, background)
    });
    let result = run(
        &document(vec![still_occurrence("c", glyphs.quad(4.0), 0, 9)]),
        &mut source,
        &root,
    );
    let plates = &result.texts[0].plates;
    let spans: Vec<(u64, u64)> = plates
        .iter()
        .map(|p| (p.first_frame, p.last_frame))
        .collect();
    assert_eq!(spans, [(0, 4), (5, 9)]);
    assert_eq!(plates[0].mask, plates[1].mask);
    assert_ne!(plates[0].source, plates[1].source);
    for plate in plates {
        let source = image::open(root.join(&plate.source))
            .expect("source PNG")
            .to_rgb8();
        assert_eq!(source.dimensions(), (plate.rect.width, plate.rect.height));
        assert_eq!((plate.shift, plate.scale), ([0.0, 0.0], 1.0));
    }
}

#[test]
fn held_frames_share_one_plate_and_decode_once() {
    let root = job_dir("held");
    let glyphs = Glyphs::row(200, 160, 0);
    let paint = glyphs.clone();
    let mut source = Scripted::new(10, SIZE, move |index, x, y| {
        let flicker = u8::from(index % 2 == 0);
        let [r, g, b] = Background::Gradient.at(x, y);
        paint.paint(x, y, [r + flicker, g, b])
    });
    let result = run(
        &document(vec![still_occurrence("h", glyphs.quad(4.0), 0, 9)]),
        &mut source,
        &root,
    );
    let plates = &result.texts[0].plates;
    assert_eq!(plates.len(), 1);
    assert_eq!((plates[0].first_frame, plates[0].last_frame), (0, 9));
    assert_eq!(source.requests.len(), 2, "keyframe, then the span once");
    assert_eq!((source.requests[1].1, source.requests[1].2), (0, 9));
}

fn moving_occurrence(
    id: &str,
    glyphs: &Glyphs,
    frames: u64,
    sample_every: u64,
    key: u64,
) -> TextOccurrence {
    let samples: Vec<(f64, f64, Quad)> = (0..frames)
        .step_by(sample_every as usize)
        .map(|f| {
            let end = (f + sample_every).min(frames);
            let quad = glyphs.shifted(3 * f as i64, 0).quad(4.0);
            (f as f64 / FPS, end as f64 / FPS, quad)
        })
        .collect();
    occurrence(id, &samples, (key as f64 + 0.5) / FPS)
}

#[test]
fn moving_writing_is_followed_frame_by_frame() {
    for sample_every in [1, 4] {
        let root = job_dir(&format!("moving-{sample_every}"));
        let glyphs = Glyphs::row(200, 160, 2);
        let paint = glyphs.clone();
        let mut source = Scripted::new(12, SIZE, move |index, x, y| {
            paint
                .shifted(3 * index as i64, 0)
                .paint(x, y, Background::Flat.at(x, y))
        });
        let item = moving_occurrence("m", &glyphs, 12, sample_every, 6);
        let result = run(&document(vec![item]), &mut source, &root);
        let text = &result.texts[0];
        assert_eq!(text.status, ReplaceStatus::Pending);
        assert_eq!(text.plates.len(), 12);
        for (f, plate) in text.plates.iter().enumerate() {
            assert_eq!((plate.first_frame, plate.last_frame), (f as u64, f as u64));
            assert_eq!(plate.shift, [3.0 * (f as f64 - 6.0), 0.0], "frame {f}");
            assert_eq!(plate.scale, 1.0);
            let mask = read_mask(&root, &plate.mask);
            let truth = glyphs.shifted(3 * f as i64, 0).truth(plate.rect, 2);
            assert!(iou(&mask, &truth) >= 0.85, "frame {f}");
        }
        assert!(text.plates[6].mask.ends_with("mask.png"));
        assert!(text.plates[0].mask.to_string_lossy().contains("mask-"));
        assert_eq!(
            source.requests.len(),
            3,
            "keyframe, following, then the plates"
        );
    }
}

#[test]
fn scrambled_moving_writing_falls_back() {
    let root = job_dir("scrambled");
    let glyphs = Glyphs::row(200, 160, 2);
    let paint = glyphs.clone();
    let mut source = Scripted::new(12, SIZE, move |index, x, y| {
        if index != 6 && (150..400).contains(&x) && (120..240).contains(&y) {
            let h = hash(u64::from(x), u64::from(y), index);
            return [h as u8, (h >> 8) as u8, (h >> 16) as u8];
        }
        paint
            .shifted(3 * index as i64, 0)
            .paint(x, y, Background::Flat.at(x, y))
    });
    let item = moving_occurrence("s", &glyphs, 12, 1, 6);
    let result = run(&document(vec![item]), &mut source, &root);
    assert_eq!(
        result.texts[0].status,
        ReplaceStatus::Fallback(super::follow::UNFOLLOWED.into())
    );
    assert!(result.texts[0].plates.is_empty());
    assert!(!root.join("visual/masks/s").exists());
}

#[test]
fn a_restless_background_falls_back_after_two_thousand_plates() {
    let root = job_dir("restless");
    let glyphs = Glyphs {
        strokes: vec![(70, 40, 90, 46), (77, 40, 83, 55)],
        outline: 0,
    };
    let paint = glyphs.clone();
    let frames = 2001;
    let mut source = Scripted::new(frames, (160, 90), move |index, x, y| {
        let i = index as u32;
        let background = [(i * 7 % 90) as u8, (i * 13 % 90) as u8, (i * 29 % 90) as u8];
        paint.paint(x, y, background)
    });
    let item = still_occurrence("r", glyphs.quad(3.0), 0, frames as u64 - 1);
    let mut text = document(vec![item]);
    text.width = 160;
    text.height = 90;
    let result = run(&text, &mut source, &root);
    assert_eq!(
        result.texts[0].status,
        ReplaceStatus::Fallback(super::plates::TOO_OFTEN.into())
    );
    assert!(result.texts[0].plates.is_empty());
    assert!(!root.join("visual/masks/r").exists());
}

#[test]
fn a_rerun_clears_stale_mask_folders() {
    let root = job_dir("rerun");
    let stale = root.join("visual/masks/stale");
    std::fs::create_dir_all(&stale).expect("create a stale folder");
    let mut source = Scripted::new(1, SIZE, |_, _, _| [0, 0, 0]);
    let result = run(&document(Vec::new()), &mut source, &root);
    assert!(result.texts.is_empty());
    assert!(!stale.exists());
}
