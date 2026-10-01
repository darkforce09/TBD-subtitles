//! The localized video's replacements: keyframe plates, mask placement, status words, pictures.

use std::fs;

use job_model::job::JobSettings;
use job_model::onscreen::{ReplacementDocument, TextCheck, TextKeyframe};

use super::*;

fn occurrence(id: &str, start_s: f64, end_s: f64, keyframe_s: Option<f64>) -> TextOccurrence {
    TextOccurrence {
        ruby: Vec::new(),
        source_fingerprint: None,
        id: id.into(),
        start_s,
        end_s,
        japanese: "看板".into(),
        english: Some("Sign".into()),
        confidence: 0.9,
        crops: Vec::new(),
        frames: Vec::new(),
        provenance: Default::default(),
        presentation: Default::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: Some(true),
        keyframe: keyframe_s.map(|time_s| TextKeyframe {
            time_s,
            image: PathBuf::from("visual/keyframes/a.png"),
        }),
    }
}

fn plate(first_frame: u64, last_frame: u64, x: u32, mask: &str) -> Plate {
    Plate {
        first_frame,
        last_frame,
        rect: PixelRect {
            x,
            y: 540,
            width: 480,
            height: 270,
        },
        shift: [0.0, 0.0],
        scale: 1.0,
        source: PathBuf::from("visual/masks/a/source.png"),
        mask: PathBuf::from(mask),
        plate: None,
        patch: None,
        shifted: Vec::new(),
    }
}

fn replaced(id: &str, status: ReplaceStatus, plates: Vec<Plate>) -> ReplacedText {
    ReplacedText {
        id: id.into(),
        first_frame: 240,
        last_frame: 287,
        status,
        style: None,
        container: None,
        plates,
        preview: Some(PathBuf::from(format!("visual/patches/{id}/preview.png"))),
        lettering_quad: None,
    }
}

#[test]
fn the_keyframe_frame_is_its_share_of_the_span_else_the_middle_else_the_first() {
    let text = replaced("a", ReplaceStatus::Baked, Vec::new());
    let keyed = occurrence("a", 10.0, 12.0, Some(11.5));
    assert_eq!(
        keyframe_frame(&text, Some(&keyed)),
        240 + 36,
        "three quarters in"
    );
    let middle = occurrence("a", 10.0, 12.0, None);
    assert_eq!(keyframe_frame(&text, Some(&middle)), 240 + 24);
    let late = occurrence("a", 10.0, 12.0, Some(50.0));
    assert_eq!(
        keyframe_frame(&text, Some(&late)),
        287,
        "held to the last frame"
    );
    let empty = occurrence("a", 10.0, 10.0, Some(10.0));
    assert_eq!(keyframe_frame(&text, Some(&empty)), 240);
    assert_eq!(keyframe_frame(&text, None), 240);
}

#[test]
fn the_keyframe_plate_covers_the_frame_else_is_the_nearest() {
    let plates = [
        plate(240, 259, 0, "visual/masks/a/0.png"),
        plate(270, 287, 100, "visual/masks/a/1.png"),
    ];
    assert_eq!(keyframe_plate(&plates, 250).map(|p| p.rect.x), Some(0));
    assert_eq!(keyframe_plate(&plates, 275).map(|p| p.rect.x), Some(100));
    assert_eq!(keyframe_plate(&plates, 268).map(|p| p.rect.x), Some(100));
    assert_eq!(keyframe_plate(&plates, 261).map(|p| p.rect.x), Some(0));
    assert!(keyframe_plate(&[], 250).is_none());
}

#[test]
fn a_plate_rectangle_maps_to_shares_of_the_picture() {
    let rect = PixelRect {
        x: 480,
        y: 270,
        width: 960,
        height: 270,
    };
    assert_eq!(mask_area(rect, 1920, 1080), Some([0.25, 0.25, 0.75, 0.5]));
    let past = PixelRect {
        x: 1800,
        y: 0,
        width: 400,
        height: 10,
    };
    assert_eq!(mask_area(past, 1920, 1080).map(|a| a[2]), Some(1.0));
    assert_eq!(mask_area(rect, 0, 1080), None);
    assert_eq!(mask_area(PixelRect::default(), 1920, 1080), None);
}

#[test]
fn the_status_says_replaced_or_why_not() {
    let with = |status: ReplaceStatus| Replacement {
        status,
        preview: None,
        mask: None,
        check: None,
    };
    assert_eq!(
        status_line(Some(&with(ReplaceStatus::Baked))),
        "Replaced in the video"
    );
    assert_eq!(
        status_line(Some(&with(ReplaceStatus::Fallback(
            "the writing moves too fast".into()
        )))),
        "Not replaced in the video: the writing moves too fast"
    );
    for pending in [Some(&with(ReplaceStatus::Pending)), None] {
        assert!(status_line(pending).starts_with("Not replaced in the video: "));
    }
    assert_eq!(default_mode(), PreviewMode::Localized);
}

#[test]
fn replacements_take_the_keyframe_plate_s_mask_and_keep_paths_inside_the_job() {
    let root = Path::new("/work/job");
    let document = TextDocument {
        occurrences: vec![occurrence("a", 10.0, 12.0, Some(11.5))],
        width: 1920,
        height: 1080,
        ..TextDocument::default()
    };
    let mut escaping = replaced("b", ReplaceStatus::Fallback("busy".into()), Vec::new());
    escaping.preview = Some(PathBuf::from("../../etc/passwd"));
    let composed = ReplacementDocument {
        width: 1920,
        height: 1080,
        frame_count: 720,
        texts: vec![
            replaced(
                "a",
                ReplaceStatus::Baked,
                vec![
                    plate(240, 259, 0, "visual/masks/a/0.png"),
                    plate(260, 287, 100, "visual/masks/a/1.png"),
                ],
            ),
            escaping,
        ],
    };
    let verified = VerifiedReplacements {
        document: composed,
        checks: vec![TextCheck {
            id: "a".into(),
            samples: 1,
            passed: true,
        }],
    };
    let telling = BTreeMap::from([(
        "a".to_string(),
        VerifyReading {
            frame: 250,
            english_read: "Palace".into(),
            similarity: 1.0,
            passed: true,
            ..VerifyReading::default()
        },
    )]);
    let found = replacements(root, &verified, &telling, &document);
    assert_eq!(
        found["a"].check.as_deref(),
        Some("Checked: English reads back as “Palace”")
    );
    assert_eq!(
        found["b"].check, None,
        "a fallback before the check was not checked"
    );
    let a = &found["a"];
    assert_eq!(a.status, ReplaceStatus::Baked);
    assert_eq!(
        a.preview.as_deref(),
        Some(Path::new("/work/job/visual/patches/a/preview.png"))
    );
    let mask = a.mask.as_ref().expect("a mask");
    assert_eq!(mask.rect.x, 100, "frame 276 lies on the second plate");
    assert_eq!(mask.path, Path::new("/work/job/visual/masks/a/1.png"));
    let b = &found["b"];
    assert_eq!(b.preview, None, "a path leaving the job folder is dropped");
    assert_eq!(b.mask, None, "no plates, no mask");
}

/// A job folder under the temp folder with job settings `enabled` and `localized_video`.
fn job(name: &str, enabled: bool, localized_video: bool) -> (PathBuf, PathBuf, JobRecord) {
    let root = std::env::temp_dir().join(format!("tbd-localized-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("work/visual")).expect("work folder");
    let mut settings = JobSettings::with_glossary(Vec::new());
    settings.onscreen_text.enabled = enabled;
    settings.onscreen_text.localized_video = localized_video;
    let record = JobRecord {
        video: root.join("a.mkv").display().to_string(),
        video_size: 1,
        video_modified_s: 0,
        settings,
        models_dir: None,
        corrections: None,
    };
    (root.clone(), root.join("work"), record)
}

#[test]
fn a_job_without_the_localized_video_has_none() {
    let (root, work, record) = job("off", true, false);
    let document = TextDocument::default();
    assert!(
        load(
            &work,
            Rows::default(),
            &record,
            &OutputRecord::default(),
            &document
        )
        .is_none()
    );
    let (_, _, off) = job("off", false, true);
    assert!(
        load(
            &work,
            Rows::default(),
            &off,
            &OutputRecord::default(),
            &document
        )
        .is_none()
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_records_are_not_written_yet_and_written_ones_name_their_files() {
    let (root, work, record) = job("records", true, true);
    let document = TextDocument {
        occurrences: vec![occurrence("a", 10.0, 12.0, None)],
        ..TextDocument::default()
    };
    let review = load(
        &work,
        Rows::default(),
        &record,
        &OutputRecord::default(),
        &document,
    )
    .expect("review");
    assert_eq!((review.video, review.subtitles), (None, None));
    assert!(review.replacements.is_empty());
    assert_eq!(review.mode, PreviewMode::Localized);
    assert!(!review.show_mask);
    let mkv = root.join("a.localized.mkv");
    let ass = root.join("a.localized.ass");
    fs::write(&mkv, b"mkv").expect("mkv");
    fs::write(&ass, b"ass").expect("ass");
    let written = LocalizedVideoRecord {
        path: Some(mkv.display().to_string()),
        replaced: 1,
        ..LocalizedVideoRecord::default()
    };
    let composed = ReplacementDocument {
        texts: vec![replaced("a", ReplaceStatus::Baked, Vec::new())],
        ..ReplacementDocument::default()
    };
    let output = OutputRecord {
        localized: Some(ass.display().to_string()),
        ..OutputRecord::default()
    };
    let written_only = Rows {
        written: Some(written.clone()),
        verified: None,
        telling: BTreeMap::new(),
    };
    let review = load(&work, written_only, &record, &output, &document).expect("review");
    assert_eq!(review.video, Some(mkv));
    assert_eq!(review.subtitles, Some(ass));
    assert!(
        review.replacements.is_empty(),
        "no replacements before the read-back check"
    );
    let mut checked = composed;
    checked.texts[0].status =
        ReplaceStatus::Fallback("The finished picture still shows Japanese".into());
    let verified = VerifiedReplacements {
        document: checked,
        checks: vec![TextCheck {
            id: "a".into(),
            samples: 1,
            passed: false,
        }],
    };
    let telling = BTreeMap::from([(
        "a".to_string(),
        VerifyReading {
            frame: 3,
            japanese_found: "王宮".into(),
            english_read: "Palace 王宮".into(),
            ..VerifyReading::default()
        },
    )]);
    let rows = Rows {
        written: Some(written),
        verified: Some(verified),
        telling,
    };
    let review = load(&work, rows, &record, &output, &document).expect("review");
    let a = &review.replacements["a"];
    assert_eq!(
        status_line(Some(a)),
        "Not replaced in the video: The finished picture still shows Japanese"
    );
    assert_eq!(
        a.check.as_deref(),
        Some("Checked: Japanese still reads “王宮”")
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pictures_decode_the_plate_and_mask_within_bounds() {
    let (root, work, _) = job("pictures", true, true);
    let preview = work.join("preview.png");
    image::RgbImage::from_pixel(1440, 360, image::Rgb([200, 30, 30]))
        .save(&preview)
        .expect("preview");
    let mask_path = work.join("mask.png");
    let mut mask = image::GrayImage::new(40, 20);
    mask.put_pixel(3, 4, image::Luma([255]));
    mask.save(&mask_path).expect("mask");
    let rect = PixelRect {
        x: 10,
        y: 20,
        width: 40,
        height: 20,
    };
    let replacement = Replacement {
        status: ReplaceStatus::Baked,
        preview: Some(preview),
        mask: Some(MaskPlate {
            rect,
            path: mask_path,
        }),
        check: None,
    };
    let pictures = pictures("a", &replacement);
    assert_eq!(pictures.id, "a");
    let plate = pictures.preview.expect("plate");
    assert_eq!(
        (plate.width, plate.height),
        (720, 180),
        "shrunk to 720 wide"
    );
    assert_eq!(plate.rgb.len(), 720 * 180 * 3);
    let mask = pictures.mask.expect("mask");
    assert_eq!((mask.width, mask.height, mask.rect), (40, 20, rect));
    assert_eq!(mask.coverage[4 * 40 + 3], 255);
    assert_eq!(mask.coverage.iter().filter(|&&c| c > 0).count(), 1);
    let gone = Replacement {
        status: ReplaceStatus::Pending,
        preview: Some(root.join("missing.png")),
        mask: None,
        check: None,
    };
    let none = super::pictures("b", &gone);
    assert!(none.preview.is_none() && none.mask.is_none());
    let _ = fs::remove_dir_all(root);
}
