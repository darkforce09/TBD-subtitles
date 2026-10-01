use std::path::PathBuf;

use job_model::job::{StepMeasure, StepRecord};
use job_model::onscreen::{
    PixelRect, Plate, ReplaceStatus, ReplacedText, TextKeyframe, TextOccurrence,
};

use super::*;
use crate::work_dir::store::scratch::Scratch;

fn occurrence(id: &str, crops: &[&str], keyframe: Option<&str>) -> TextOccurrence {
    TextOccurrence {
        id: id.into(),
        start_s: 1.0,
        end_s: 2.0,
        japanese: "海".into(),
        english: None,
        confidence: 0.9,
        crops: crops.iter().map(PathBuf::from).collect(),
        frames: Vec::new(),
        provenance: Default::default(),
        presentation: Default::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: keyframe.map(|image| TextKeyframe {
            time_s: 1.5,
            image: image.into(),
        }),
        ruby: Vec::new(),
    }
}

fn detection(occurrences: Vec<TextOccurrence>) -> TextDocument {
    TextDocument {
        occurrences,
        ..TextDocument::default()
    }
}

fn replacement(id: &str) -> ReplacementDocument {
    let plate = Plate {
        first_frame: 0,
        last_frame: 3,
        rect: PixelRect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        },
        shift: [0.0, 0.0],
        scale: 1.0,
        source: format!("visual/masks/{id}-source.png").into(),
        mask: format!("visual/masks/{id}-mask.png").into(),
        plate: Some(format!("visual/plates/{id}.png").into()),
        patch: None,
        shifted: Vec::new(),
    };
    ReplacementDocument {
        width: 8,
        height: 8,
        frame_count: 10,
        texts: vec![ReplacedText {
            id: id.into(),
            first_frame: 0,
            last_frame: 3,
            status: ReplaceStatus::Baked,
            style: None,
            container: None,
            plates: vec![plate],
            preview: Some(format!("visual/patches/{id}-preview.png").into()),
            lettering_quad: None,
        }],
    }
}

fn detect_files(scratch: &Scratch) -> NamedFiles {
    named_files(
        StepName::TextDetect,
        &scratch.store().read().unwrap(),
        scratch.work(),
    )
    .unwrap()
}

fn record() -> StepRecord {
    StepRecord {
        fingerprint: "f".into(),
        finished_ns: 1,
        measure: StepMeasure::default(),
    }
}

#[test]
fn a_missing_crop_invalidates_detection_until_its_producer_restores_it() {
    let scratch = Scratch::new("files-missing-crop");
    let store = scratch.store();
    store
        .put_output(
            StepName::TextDetect,
            None,
            &detection(vec![
                occurrence("t1", &["visual/crops/first.png"], None),
                occurrence("t2", &["visual/crops/second.png"], None),
            ]),
        )
        .unwrap();
    scratch.file("visual/crops/first.png", b"first crop");
    let second = scratch.file("visual/crops/second.png", b"second crop");
    assert!(detect_files(&scratch).present());
    std::fs::remove_file(&second).unwrap();
    assert!(!detect_files(&scratch).present());
    scratch.file("visual/crops/second.png", b"regenerated crop");
    assert!(detect_files(&scratch).present());
}

#[test]
fn malformed_references_and_occurrences_without_a_crop_cannot_resume_detection() {
    let scratch = Scratch::new("files-malformed");
    let store = scratch.store();
    scratch.file("visual/crops/empty.png", b"");
    let good = scratch.file("visual/crops/good.png", b"a crop");
    let good = good.to_string_lossy().into_owned();
    for crops in [
        vec![],
        vec!["visual/crops/empty.png"],
        vec!["visual/crops"],
        vec!["visual/crops/../crops/good.png"],
        vec![good.as_str()],
    ] {
        store
            .put_output(
                StepName::TextDetect,
                None,
                &detection(vec![occurrence("t1", &crops, None)]),
            )
            .unwrap();
        assert!(!detect_files(&scratch).present(), "{crops:?}");
    }
    store
        .put_output(StepName::TextDetect, None, &TextDocument::default())
        .unwrap();
    assert!(
        detect_files(&scratch).present(),
        "an empty detection names nothing"
    );
}

#[test]
fn a_named_keyframe_must_be_a_non_empty_file() {
    let scratch = Scratch::new("files-keyframe");
    let store = scratch.store();
    scratch.file("visual/crops/first.png", b"a crop");
    store
        .put_output(
            StepName::TextDetect,
            None,
            &detection(vec![occurrence(
                "t1",
                &["visual/crops/first.png"],
                Some("visual/keyframes/frame-00000036.png"),
            )]),
        )
        .unwrap();
    assert!(!detect_files(&scratch).present());
    let still = scratch.file("visual/keyframes/frame-00000036.png", b"");
    assert!(!detect_files(&scratch).present(), "an empty still");
    std::fs::write(&still, b"a whole-frame still").unwrap();
    assert!(detect_files(&scratch).present());
}

#[test]
fn replacements_name_their_plates_and_the_outputs_the_files_beside_the_video() {
    let scratch = Scratch::new("files-named");
    let store = scratch.store();
    let mut composed = replacement("t1");
    composed.texts[0].plates[0].patch = Some("visual/patches/t1/0.png".into());
    composed.texts[0].plates[0].shifted = vec![job_model::onscreen::ShiftedPatch {
        shift: [1.0, 0.0],
        patch: "visual/patches/t1/0-1.png".into(),
    }];
    store
        .put_output(StepName::TextCompose, None, &composed)
        .unwrap();
    let subtitles = scratch.file("beside/episode.ass", b"");
    store
        .put_output(
            StepName::Output,
            None,
            &job_model::outputs::OutputRecord {
                path: subtitles.to_string_lossy().into_owned(),
                ..Default::default()
            },
        )
        .unwrap();
    let read = store.read().unwrap();
    let composed = named_files(StepName::TextCompose, &read, scratch.work()).unwrap();
    let root = scratch.dir.clone();
    assert_eq!(
        composed.in_job,
        vec![
            root.join("visual/masks/t1-source.png"),
            root.join("visual/masks/t1-mask.png"),
            root.join("visual/plates/t1.png"),
            root.join("visual/patches/t1/0.png"),
            root.join("visual/patches/t1/0-1.png"),
            root.join("visual/patches/t1-preview.png"),
        ]
    );
    let output = named_files(StepName::Output, &read, scratch.work()).unwrap();
    assert_eq!(output.beside, vec![subtitles.clone()]);
    assert!(output.present(), "an empty subtitle file still counts");
    std::fs::remove_file(&subtitles).unwrap();
    assert!(!output.present());
    let probe = named_files(StepName::ProbeDecode, &read, scratch.work()).unwrap();
    assert_eq!(probe.in_job, vec![scratch.work().mix()]);
}

#[test]
fn opening_a_job_removes_the_owned_files_no_row_names_and_nothing_else() {
    let mut scratch = Scratch::new("files-orphans");
    let store = scratch.store();
    store
        .put_step_record(StepName::ProbeDecode, &record())
        .unwrap();
    store
        .put_output(
            StepName::TextDetect,
            None,
            &detection(vec![occurrence(
                "t1",
                &["visual/crops/t1.png"],
                Some("visual/keyframes/frame-1.png"),
            )]),
        )
        .unwrap();
    store
        .put_output(StepName::TextInpaint, None, &replacement("t1"))
        .unwrap();
    let kept: Vec<PathBuf> = [
        "audio/mix_16k.f32",
        "visual/crops/t1.png",
        "visual/keyframes/frame-1.png",
        "visual/masks/t1-source.png",
        "visual/masks/t1-mask.png",
        "visual/plates/t1.png",
        "visual/patches/t1-preview.png",
        "visual/readings/cache.json",
        "visual/translations/cache.json",
        "claude-cwd/claude-answer.json",
        "fix/calls/one.json",
        "logs/vad.log",
        "report.md",
        "sheet.txt",
    ]
    .iter()
    .map(|relative| scratch.file(relative, b"kept"))
    .collect();
    let orphans: Vec<PathBuf> = [
        "audio/vocals_16k.f32",
        "audio/mix_16k.f32.part",
        "visual/crops/t2.png",
        "visual/masks/nested/t3.png",
        "visual/patches/t9.png",
    ]
    .iter()
    .map(|relative| scratch.file(relative, b"orphan"))
    .collect();
    scratch.reopen();
    for path in &kept {
        assert!(path.is_file(), "{} was removed", path.display());
    }
    for path in &orphans {
        assert!(!path.exists(), "{} was kept", path.display());
    }
    let mut write = scratch.store().write().unwrap();
    write
        .remove(
            Table::Outputs,
            &keys::output_key(StepName::TextDetect, None),
        )
        .unwrap();
    write.commit().unwrap();
    scratch.reopen();
    assert!(!scratch.dir.join("visual/crops/t1.png").exists());
    assert!(!scratch.dir.join("visual/keyframes/frame-1.png").exists());
    assert!(
        scratch.dir.join("visual/plates/t1.png").is_file(),
        "the inpainting still names its plate"
    );
    assert!(scratch.dir.join("audio/mix_16k.f32").is_file());
}

#[test]
fn a_stored_mask_document_keeps_its_masks_and_an_unnamed_mask_is_removed_on_open() {
    let mut scratch = Scratch::new("files-masks");
    let mut masks = replacement("t1");
    masks.texts[0].plates[0].plate = None;
    masks.texts[0].preview = None;
    scratch
        .store()
        .put_output(StepName::TextMask, None, &masks)
        .unwrap();
    let source = scratch.file("visual/masks/t1-source.png", b"source");
    let mask = scratch.file("visual/masks/t1-mask.png", b"mask");
    let unnamed = scratch.file("visual/masks/t1/mask-3.png", b"left over");
    let named = named_files(
        StepName::TextMask,
        &scratch.store().read().unwrap(),
        scratch.work(),
    )
    .unwrap();
    assert_eq!(named.in_job, vec![source.clone(), mask.clone()]);
    assert!(named.present());
    scratch.reopen();
    assert!(
        source.is_file() && mask.is_file(),
        "the masks a record names"
    );
    assert!(!unnamed.exists(), "a mask no record names");
    std::fs::remove_file(&mask).unwrap();
    let named = named_files(
        StepName::TextMask,
        &scratch.store().read().unwrap(),
        scratch.work(),
    )
    .unwrap();
    assert!(!named.present(), "a missing mask cannot resume the step");
}

#[test]
fn the_checked_replacements_name_their_files_and_the_localized_video_its_file() {
    let scratch = Scratch::new("files-verified");
    let store = scratch.store();
    store
        .put_output(
            StepName::TextVerify,
            None,
            &VerifiedReplacements {
                document: replacement("t1"),
                checks: Vec::new(),
            },
        )
        .unwrap();
    let video = scratch.file("beside/episode.localized.mkv", b"video");
    store
        .put_output(
            StepName::LocalizedVideo,
            None,
            &LocalizedVideoRecord {
                path: Some(video.to_string_lossy().into_owned()),
                ..LocalizedVideoRecord::default()
            },
        )
        .unwrap();
    let read = store.read().unwrap();
    let root = scratch.dir.clone();
    let verified = named_files(StepName::TextVerify, &read, scratch.work()).unwrap();
    assert_eq!(
        verified.in_job,
        vec![
            root.join("visual/masks/t1-source.png"),
            root.join("visual/masks/t1-mask.png"),
            root.join("visual/plates/t1.png"),
            root.join("visual/patches/t1-preview.png"),
        ]
    );
    assert!(!verified.present());
    let localized = named_files(StepName::LocalizedVideo, &read, scratch.work()).unwrap();
    assert_eq!(localized.beside, vec![video.clone()]);
    assert!(localized.present());
    std::fs::remove_file(&video).unwrap();
    assert!(
        !localized.present(),
        "a removed localized video reruns its step"
    );
    store
        .put_output(
            StepName::LocalizedVideo,
            None,
            &LocalizedVideoRecord::default(),
        )
        .unwrap();
    let disabled = named_files(
        StepName::LocalizedVideo,
        &store.read().unwrap(),
        scratch.work(),
    )
    .unwrap();
    assert!(disabled.present(), "a job without the video names none");
}
