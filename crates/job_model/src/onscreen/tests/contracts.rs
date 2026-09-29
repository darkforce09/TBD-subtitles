use super::*;

#[test]
fn old_settings_do_not_enable_work_but_new_jobs_do() {
    assert!(!TextSettings::default().enabled);
    assert!(TextSettings::new_job().enabled);
}

#[test]
fn edits_reject_invalid_timing_and_sizes() {
    let mut edit = TextEdit {
        english: Some("Rebecca".into()),
        start_s: 1.0,
        end_s: 2.0,
        presentation: TextPresentation::default(),
        source_fingerprint: None,
    };
    assert!(edit.validate(3.0).is_ok());
    edit.end_s = 0.0;
    assert!(edit.validate(3.0).is_err());
    edit.end_s = 4.0;
    assert!(edit.validate(3.0).is_err());
    edit.end_s = 2.0;
    edit.presentation.font_size = Some(f64::NAN);
    assert!(edit.validate(3.0).is_err());
}

fn occurrence() -> TextOccurrence {
    TextOccurrence {
        id: "text-000001".into(),
        start_s: 1.0,
        end_s: 2.0,
        japanese: "レベッカ".into(),
        english: Some("Rebecca".into()),
        confidence: 0.95,
        crops: vec!["text/crop-000001.png".into()],
        frames: vec![TextFrame {
            time_s: 1.0,
            end_s: 2.0,
            quad: Quad([
                Point { x: 100.0, y: 200.0 },
                Point { x: 300.0, y: 200.0 },
                Point { x: 300.0, y: 240.0 },
                Point { x: 100.0, y: 240.0 },
            ]),
            confidence: 0.99,
            surface_rgb: Some([200, 200, 200]),
        }],
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: None,
    }
}

#[test]
fn source_identity_changes_with_observed_content_crop_geometry_and_frame_span() {
    let original = occurrence();
    let identity = original.observation_fingerprint();
    for change in 0..6 {
        let mut changed = original.clone();
        match change {
            0 => changed.japanese = "ベラミー".into(),
            1 => changed.crops[0] = "text/crop-000002.png".into(),
            2 => changed.frames[0].quad.0[0].x += 3.0,
            3 => changed.frames[0].time_s += 0.04,
            4 => changed.frames[0].end_s -= 0.04,
            5 => changed.frames.push(changed.frames[0].clone()),
            _ => unreachable!(),
        }
        assert_ne!(identity, changed.observation_fingerprint(), "case {change}");
    }
}

#[test]
fn presentation_and_review_metadata_do_not_change_observed_identity() {
    let original = occurrence();
    let mut edited = original.clone();
    edited.id = "text-000002".into();
    edited.english = Some("Gladiator Rebecca".into());
    edited.start_s = 1.1;
    edited.end_s = 1.9;
    edited.presentation = TextPresentation {
        treatment: TextTreatment::Nearby,
        anchor: Some(Point { x: 400.0, y: 500.0 }),
        font_size: Some(48.0),
    };
    edited.confidence = 0.5;
    edited.frames[0].confidence = 0.7;
    edited.frames[0].surface_rgb = None;
    edited.warnings.push("Needs tracking review".into());
    edited.provenance.backend = "owner".into();
    edited.reviewed = true;
    edited.rendered = Some(true);
    edited.source_fingerprint = Some("preserved source identity".into());
    assert_eq!(
        original.observation_fingerprint(),
        edited.observation_fingerprint()
    );
}

#[test]
fn editing_an_already_reviewed_occurrence_preserves_its_unedited_source_identity() {
    let mut text = occurrence();
    let first = TextEdit::from_occurrence(&text);
    assert_eq!(
        first.source_fingerprint.as_deref(),
        Some(text.observation_fingerprint().as_str())
    );
    text.source_fingerprint = first.source_fingerprint.clone();
    text.reviewed = true;
    text.english = Some("Gladiator Rebecca".into());
    text.start_s = 1.1;
    text.end_s = 1.9;
    text.frames[0].time_s = text.start_s;
    text.frames[0].end_s = text.end_s;
    text.presentation.anchor = Some(Point { x: 450.0, y: 300.0 });
    text.presentation.font_size = Some(48.0);
    let second = TextEdit::from_occurrence(&text);
    assert_eq!(second.source_fingerprint, first.source_fingerprint);
    assert_ne!(
        second.source_fingerprint.as_deref(),
        Some(text.observation_fingerprint().as_str())
    );
    assert_eq!(second.english, text.english);
    assert_eq!((second.start_s, second.end_s), (1.1, 1.9));
    assert_eq!(second.presentation, text.presentation);
    let round_trip: TextEdit =
        serde_json::from_slice(&serde_json::to_vec(&second).unwrap()).unwrap();
    assert_eq!(round_trip, second);
}

#[test]
fn legacy_records_load_without_fabricating_an_original_identity() {
    let mut legacy = serde_json::to_value(occurrence()).unwrap();
    legacy.as_object_mut().unwrap().remove("source_fingerprint");
    legacy["reviewed"] = true.into();
    let text: TextOccurrence = serde_json::from_value(legacy).unwrap();
    assert!(text.source_fingerprint.is_none());
    assert!(
        TextEdit::from_occurrence(&text)
            .source_fingerprint
            .is_none()
    );

    let legacy_edit: TextEdit = serde_json::from_value(serde_json::json!({
        "english": "Rebecca", "start_s": 1.0, "end_s": 2.0,
        "presentation": {"treatment": "auto", "anchor": null, "font_size": null}
    }))
    .unwrap();
    assert!(legacy_edit.source_fingerprint.is_none());
    let legacy_document: TextDocument = serde_json::from_value(serde_json::json!({
        "width": 1920, "height": 1080, "decoded_frames": 24, "occurrences": []
    }))
    .unwrap();
    assert!(legacy_document.review_warnings.is_empty());
}

#[test]
fn document_level_review_warnings_remain_visible_even_without_occurrences() {
    let document = TextDocument {
        review_warnings: vec!["Correction text-000001 no longer matches a sign".into()],
        ..TextDocument::default()
    };
    let summary = document.summary();
    assert_eq!(summary.detected, 0);
    assert_eq!(summary.translated, 0);
    assert_eq!(summary.flagged, 1);
}

#[test]
fn edit_validation_rejects_nonfinite_and_out_of_range_fields() {
    for (start_s, end_s) in [
        (f64::NAN, 2.0),
        (f64::NEG_INFINITY, 2.0),
        (-0.01, 2.0),
        (1.0, f64::NAN),
        (1.0, f64::INFINITY),
        (1.0, 1.0),
        (2.0, 1.0),
        (1.0, 3.01),
    ] {
        let mut edit = TextEdit::from_occurrence(&occurrence());
        edit.start_s = start_s;
        edit.end_s = end_s;
        assert!(edit.validate(3.0).is_err(), "span {start_s}..{end_s}");
    }
    for size in [f64::NAN, f64::INFINITY, 7.99, 400.01] {
        let mut edit = TextEdit::from_occurrence(&occurrence());
        edit.presentation.font_size = Some(size);
        assert!(edit.validate(3.0).is_err(), "size {size}");
    }
    for anchor in [
        Point {
            x: f64::NAN,
            y: 0.0,
        },
        Point {
            x: 0.0,
            y: f64::INFINITY,
        },
    ] {
        let mut edit = TextEdit::from_occurrence(&occurrence());
        edit.presentation.anchor = Some(anchor);
        assert!(edit.validate(3.0).is_err());
    }
    for size in [8.0, 400.0] {
        let mut edit = TextEdit::from_occurrence(&occurrence());
        edit.start_s = 0.0;
        edit.end_s = 3.0;
        edit.presentation.font_size = Some(size);
        assert!(edit.validate(3.0).is_ok());
    }
}
