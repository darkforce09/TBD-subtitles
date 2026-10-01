use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use super::{
    DetectorEngine, LetteringStyle, LibrarySign, LocalizedEncoder, LocalizedVideoRecord, PixelRect,
    Plate, Point, Quad, ReplaceStatus, ReplacedText, ReplacementDocument, SegmentSummary,
    ShiftedPatch, TextCheck, TextCorrections, TextDocument, TextEdit, TextFrame, TextKeyframe,
    TextOccurrence, TextPresentation, TextProvenance, TextSettings, TextSummary, TextTreatment,
    VerifiedReplacements, VerifyReading,
};
use crate::archive_round_trip::round_trip;

const EVERY_TREATMENT: [TextTreatment; 3] = [
    TextTreatment::Auto,
    TextTreatment::Replace,
    TextTreatment::Nearby,
];

/// Fails to compile when `TextTreatment` gains a variant, until [`EVERY_TREATMENT`] lists it too.
fn listed_treatment(treatment: &TextTreatment) {
    match treatment {
        TextTreatment::Auto | TextTreatment::Replace | TextTreatment::Nearby => {}
    }
}

fn path(name: &str) -> PathBuf {
    PathBuf::from(format!("/media/one pace/ドレスローザ/{name}"))
}

fn quad() -> Quad {
    Quad([
        Point { x: 10.0, y: 20.0 },
        Point { x: 110.0, y: 20.0 },
        Point { x: 110.0, y: 60.0 },
        Point { x: 10.0, y: 60.0 },
    ])
}

fn frame() -> TextFrame {
    TextFrame {
        time_s: 12.5,
        end_s: 13.0,
        quad: quad(),
        confidence: 0.9,
        surface_rgb: Some([250, 240, 230]),
    }
}

fn presentation(treatment: TextTreatment) -> TextPresentation {
    TextPresentation {
        treatment,
        anchor: Some(Point { x: 960.0, y: 100.0 }),
        font_size: Some(48.0),
    }
}

fn provenance() -> TextProvenance {
    TextProvenance {
        backend: "claude".into(),
        reference: Some(path("reference 1.txt")),
        reason: "read from the keyframe".into(),
    }
}

fn occurrence(treatment: TextTreatment) -> TextOccurrence {
    TextOccurrence {
        id: "T0001".into(),
        start_s: 12.5,
        end_s: 15.0,
        japanese: "ドレスローザ".into(),
        english: Some("Dressrosa".into()),
        confidence: 0.95,
        crops: vec![path("crop 1.png"), path("crop 2.png")],
        frames: vec![frame()],
        provenance: provenance(),
        presentation: presentation(treatment),
        warnings: vec!["faint".into()],
        reviewed: true,
        rendered: Some(true),
        source_fingerprint: Some("v1-abc".into()),
        keyframe: Some(keyframe()),
        ruby: vec![quad()],
    }
}

fn keyframe() -> TextKeyframe {
    TextKeyframe {
        time_s: 13.0,
        image: path("keyframe 1.png"),
    }
}

fn edit() -> TextEdit {
    TextEdit {
        english: Some("Dressrosa".into()),
        start_s: 12.5,
        end_s: 15.0,
        presentation: presentation(TextTreatment::Replace),
        source_fingerprint: Some("v1-abc".into()),
    }
}

fn settings() -> TextSettings {
    TextSettings {
        enabled: true,
        claude_fallback: false,
        reference_folder: Some(path("reference")),
        localized_video: true,
        hardware_decode: true,
        detector_engine: DetectorEngine::TensorRt,
        localized_encoder: LocalizedEncoder::Nvenc,
    }
}

fn rect() -> PixelRect {
    PixelRect {
        x: 8,
        y: 16,
        width: 128,
        height: 64,
    }
}

fn every_status() -> Vec<ReplaceStatus> {
    vec![
        ReplaceStatus::Pending,
        ReplaceStatus::Baked,
        ReplaceStatus::Fallback("the writing crosses a face".into()),
    ]
}

/// Fails to compile when `ReplaceStatus` gains a variant, until [`every_status`] lists it too.
fn listed_status(status: &ReplaceStatus) {
    match status {
        ReplaceStatus::Pending | ReplaceStatus::Baked | ReplaceStatus::Fallback(_) => {}
    }
}

fn style() -> LetteringStyle {
    LetteringStyle {
        fill_rgb: [255, 255, 255],
        outline_rgb: Some([0, 0, 0]),
        outline_px: 2.5,
        soft_outline: true,
        stroke_px: 4.0,
        line_height_px: 40.0,
    }
}

fn plate() -> Plate {
    Plate {
        first_frame: 300,
        last_frame: 330,
        rect: rect(),
        shift: [1.5, -2.0],
        scale: 1.05,
        source: path("plate 1 source.png"),
        mask: path("plate 1 mask.png"),
        plate: Some(path("plate 1.png")),
        patch: Some(path("plate 1 patch.png")),
        shifted: vec![ShiftedPatch {
            shift: [2.5, -2.0],
            patch: path("plate 1 patch 1.png"),
        }],
    }
}

fn replaced(status: ReplaceStatus) -> ReplacedText {
    ReplacedText {
        id: "T0001".into(),
        first_frame: 300,
        last_frame: 360,
        status,
        style: Some(style()),
        container: Some("card 1".into()),
        plates: vec![plate()],
        preview: Some(path("preview 1.png")),
        lettering_quad: Some(quad()),
    }
}

fn document() -> ReplacementDocument {
    ReplacementDocument {
        width: 1920,
        height: 1080,
        frame_count: 34_000,
        texts: every_status().into_iter().map(replaced).collect(),
    }
}

fn reading() -> VerifyReading {
    VerifyReading {
        frame: 312,
        japanese_found: "海".into(),
        english_read: "DRESSROSA".into(),
        similarity: 0.875,
        passed: true,
    }
}

fn check() -> TextCheck {
    TextCheck {
        id: "T0001".into(),
        samples: 1,
        passed: true,
    }
}

#[test]
fn point_and_quad_round_trip() {
    round_trip(&Point { x: 1.5, y: -2.5 });
    round_trip(&quad());
}

#[test]
fn text_frame_round_trips() {
    round_trip(&frame());
}

#[test]
fn every_text_treatment_round_trips() {
    for treatment in EVERY_TREATMENT {
        listed_treatment(&treatment);
        round_trip(&treatment);
    }
}

#[test]
fn text_presentation_round_trips() {
    for treatment in EVERY_TREATMENT {
        round_trip(&presentation(treatment));
    }
}

#[test]
fn text_provenance_round_trips() {
    round_trip(&provenance());
}

#[test]
fn text_occurrence_round_trips() {
    for treatment in EVERY_TREATMENT {
        round_trip(&occurrence(treatment));
    }
}

#[test]
fn text_keyframe_round_trips() {
    round_trip(&keyframe());
}

#[test]
fn text_document_round_trips() {
    round_trip(&TextDocument {
        width: 1920,
        height: 1080,
        decoded_frames: 34_000,
        occurrences: EVERY_TREATMENT.into_iter().map(occurrence).collect(),
        review_warnings: vec!["orphaned correction T0099".into()],
        proxy_width: 640,
        sample_step: 12,
    });
}

#[test]
fn text_edit_round_trips() {
    round_trip(&edit());
}

#[test]
fn text_corrections_round_trip() {
    round_trip(&TextCorrections {
        edits: BTreeMap::from([("T0001".into(), edit()), ("T0002".into(), edit())]),
        retry: vec!["T0003".into()],
    });
}

#[test]
fn text_summary_round_trips() {
    round_trip(&TextSummary {
        detected: 21,
        translated: 20,
        fallback: 3,
        unresolved: 1,
        flagged: 2,
    });
}

#[test]
fn text_settings_round_trip() {
    round_trip(&settings());
}

#[test]
fn pixel_rect_round_trips() {
    round_trip(&rect());
}

#[test]
fn every_replace_status_round_trips() {
    for status in every_status() {
        listed_status(&status);
        round_trip(&status);
    }
}

#[test]
fn lettering_style_round_trips() {
    round_trip(&style());
}

#[test]
fn plate_round_trips() {
    round_trip(&plate());
}

#[test]
fn replaced_text_round_trips() {
    for status in every_status() {
        round_trip(&replaced(status));
    }
}

#[test]
fn replacement_document_round_trips() {
    round_trip(&document());
}

#[test]
fn localized_video_record_round_trips() {
    round_trip(&LocalizedVideoRecord {
        path: Some("/media/one pace/Dressrosa 11.localized.mkv".into()),
        encoder: "hevc_nvenc".into(),
        frames: 34_000,
        replaced: 15,
        earlier: Some("/media/one pace/Dressrosa 11.localized.old.mkv".into()),
        segments: SegmentSummary {
            segments_reencoded: 4,
            frames_reencoded: 1_200,
            frames_copied: 32_800,
            fallback_reason: None,
        },
    });
}

#[test]
fn verify_types_round_trip() {
    round_trip(&reading());
    round_trip(&check());
    round_trip(&VerifiedReplacements {
        document: document(),
        checks: vec![check()],
    });
}

#[test]
fn a_path_that_is_not_utf8_fails_to_archive() {
    let mut item = keyframe();
    item.image = PathBuf::from(OsStr::from_bytes(b"/tmp/\xff"));
    assert!(rkyv::to_bytes::<rkyv::rancor::Error>(&item).is_err());

    let mut item = occurrence(TextTreatment::Auto);
    item.crops
        .push(PathBuf::from(OsStr::from_bytes(b"/tmp/\xff")));
    assert!(rkyv::to_bytes::<rkyv::rancor::Error>(&item).is_err());
}

#[test]
fn library_sign_round_trips_in_rkyv_and_json() {
    let sign = LibrarySign {
        japanese: "ドレスローザ王宮".into(),
        crop_hash: 0x0f0f_00ff_1234_8001,
        english: "Dressrosa Royal Palace".into(),
        confidence: 0.93,
        style: style(),
        patch_png: vec![0x89, b'P', b'N', b'G', 1, 2, 3],
        mask_png: vec![0x89, b'P', b'N', b'G', 255, 0],
        episodes: vec![
            "dressrosa-11-0a1b2c3d".into(),
            "dressrosa-28-9f8e7d6c".into(),
        ],
        added_s: 1_790_000_000,
    };
    round_trip(&sign);
    let json = serde_json::to_string(&sign).unwrap();
    assert_eq!(serde_json::from_str::<LibrarySign>(&json).unwrap(), sign);
    assert_eq!(sign.origin(), Some("dressrosa-11-0a1b2c3d"));
}
