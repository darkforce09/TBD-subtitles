use super::apply;
use job_model::onscreen::{
    Point, Quad, TextCorrections, TextDocument, TextEdit, TextFrame, TextOccurrence,
    TextPresentation, TextProvenance, TextTreatment,
};

fn occurrence() -> TextOccurrence {
    let quad = Quad([
        Point { x: 100.0, y: 200.0 },
        Point { x: 300.0, y: 200.0 },
        Point { x: 300.0, y: 240.0 },
        Point { x: 100.0, y: 240.0 },
    ]);
    TextOccurrence {
        id: "text-000001".into(),
        start_s: 1.0,
        end_s: 3.0,
        japanese: "レベッカ".into(),
        english: Some("Rebecca".into()),
        confidence: 0.95,
        crops: vec!["text/crop-000001.png".into()],
        frames: (1..3)
            .map(|second| TextFrame {
                time_s: f64::from(second),
                end_s: f64::from(second + 1),
                quad,
                confidence: 0.99,
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
    }
}

fn document(text: TextOccurrence) -> TextDocument {
    TextDocument {
        width: 1920,
        height: 1080,
        decoded_frames: 48,
        occurrences: vec![text],
        review_warnings: Vec::new(),
        proxy_width: 0,
        sample_step: 0,
    }
}

fn corrections(text: &TextOccurrence) -> TextCorrections {
    let mut edit = TextEdit::from_occurrence(text);
    edit.english = Some("Gladiator Rebecca".into());
    edit.start_s = 1.25;
    edit.end_s = 2.75;
    edit.presentation = TextPresentation {
        treatment: TextTreatment::Nearby,
        anchor: Some(Point { x: 500.0, y: 600.0 }),
        font_size: Some(48.0),
    };
    TextCorrections {
        edits: [(text.id.clone(), edit)].into(),
        retry: Vec::new(),
    }
}

#[test]
fn matching_correction_applies_and_a_second_edit_keeps_the_original_source_identity() {
    let original = occurrence();
    let corrections = corrections(&original);
    let edit = &corrections.edits[&original.id];
    let mut input = document(original.clone());
    apply(&mut input, &corrections, 10.0).unwrap();
    let text = &input.occurrences[0];
    assert!(text.reviewed);
    assert_eq!(text.english, edit.english);
    assert_eq!((text.start_s, text.end_s), (1.25, 2.75));
    assert_eq!(text.presentation, edit.presentation);
    assert_eq!(text.frames[0].time_s, 1.25);
    assert_eq!(text.frames[1].end_s, 2.75);
    assert_eq!(text.source_fingerprint, edit.source_fingerprint);
    assert_ne!(
        text.source_fingerprint.as_deref(),
        Some(text.observation_fingerprint().as_str())
    );
    assert!(text.warnings.is_empty());

    let input: TextDocument = serde_json::from_slice(&serde_json::to_vec(&input).unwrap()).unwrap();
    let mut second = TextEdit::from_occurrence(&input.occurrences[0]);
    second.english = Some("Rebecca of Corrida Colosseum".into());
    second.start_s = 1.5;
    second.end_s = 2.5;
    second.presentation.anchor = Some(Point { x: 550.0, y: 580.0 });
    assert_eq!(second.source_fingerprint, edit.source_fingerprint);
    let second_corrections = TextCorrections {
        edits: [(original.id.clone(), second.clone())].into(),
        retry: Vec::new(),
    };
    for mut input in [input, document(original)] {
        apply(&mut input, &second_corrections, 10.0).unwrap();
        let text = &input.occurrences[0];
        assert!(text.reviewed);
        assert_eq!(text.english, second.english);
        assert_eq!((text.start_s, text.end_s), (1.5, 2.5));
        assert_eq!(text.presentation, second.presentation);
        assert_eq!(text.source_fingerprint, second.source_fingerprint);
        assert!(text.warnings.is_empty());
    }
}

#[test]
fn changed_japanese_crop_geometry_or_observed_span_rejects_a_stale_edit() {
    let original = occurrence();
    let corrections = corrections(&original);
    for change in 0..5 {
        let mut changed = original.clone();
        changed.source_fingerprint = Some(original.observation_fingerprint());
        changed.rendered = Some(true);
        match change {
            0 => changed.japanese = "ベラミー".into(),
            1 => changed.crops[0] = "text/crop-000099.png".into(),
            2 => changed.frames[1].quad.0[2].x += 5.0,
            3 => {
                changed.start_s = 1.1;
                changed.frames[0].time_s = changed.start_s;
            }
            4 => {
                changed.end_s = 2.9;
                changed.frames[1].end_s = changed.end_s;
            }
            _ => unreachable!(),
        }
        let mut input = document(changed.clone());
        apply(&mut input, &corrections, 10.0).unwrap();
        let text = &input.occurrences[0];
        assert!(!text.reviewed, "case {change}");
        assert_eq!(text.rendered, None);
        assert_eq!(text.english, changed.english);
        assert_eq!((text.start_s, text.end_s), (changed.start_s, changed.end_s));
        assert_eq!(text.frames, changed.frames);
        assert_eq!(text.presentation, changed.presentation);
        assert!(text.warnings.iter().any(|warning| {
            warning.contains("Visual correction not applied:") && warning.contains("changed")
        }));
        assert_eq!(input.summary().flagged, 1);
    }
}

#[test]
fn a_new_earlier_occurrence_cannot_inherit_a_sequential_ids_correction() {
    let original = occurrence();
    let corrections = corrections(&original);
    let mut earlier = occurrence();
    earlier.japanese = "作戦".into();
    earlier.english = Some("Operation".into());
    earlier.start_s = 0.0;
    earlier.end_s = 1.0;
    earlier.frames.truncate(1);
    earlier.frames[0].time_s = 0.0;
    earlier.frames[0].end_s = 1.0;
    let mut shifted = original;
    shifted.id = "text-000002".into();
    let mut input = document(earlier);
    input.occurrences.push(shifted);
    apply(&mut input, &corrections, 10.0).unwrap();
    assert!(input.occurrences.iter().all(|text| !text.reviewed));
    assert_eq!(input.occurrences[0].english.as_deref(), Some("Operation"));
    assert_eq!(input.occurrences[1].english.as_deref(), Some("Rebecca"));
    assert!(!input.occurrences[0].warnings.is_empty());
    assert!(input.occurrences[1].warnings.is_empty());
    assert_eq!(input.summary().flagged, 1);
}

#[test]
fn legacy_unbound_corrections_are_flagged_and_never_mark_generated_text_reviewed() {
    let original = occurrence();
    let mut corrections = corrections(&original);
    corrections
        .edits
        .get_mut(&original.id)
        .unwrap()
        .source_fingerprint = None;
    let mut input = document(original);
    apply(&mut input, &corrections, 10.0).unwrap();
    let text = &input.occurrences[0];
    assert!(!text.reviewed);
    assert_eq!(text.english.as_deref(), Some("Rebecca"));
    assert_eq!((text.start_s, text.end_s), (1.0, 3.0));
    assert!(
        text.warnings
            .iter()
            .any(|warning| warning.contains("identity is missing"))
    );
    assert_eq!(input.summary().flagged, 1);
}

#[test]
fn legacy_reviewed_artifacts_do_not_reuse_potentially_misattributed_english() {
    let mut legacy = occurrence();
    let corrections = corrections(&legacy);
    legacy.reviewed = true;
    legacy.rendered = Some(true);
    legacy.english = Some("A previously saved correction".into());
    let mut input = document(legacy);
    apply(&mut input, &corrections, 10.0).unwrap();
    let text = &input.occurrences[0];
    assert!(!text.reviewed);
    assert!(text.english.is_none());
    assert_eq!(text.rendered, None);
    assert!(
        text.warnings
            .iter()
            .any(|warning| warning.contains("identity is missing"))
    );
    assert_eq!(input.summary().unresolved, 1);
    assert_eq!(input.summary().flagged, 1);
}

#[test]
fn orphaned_corrections_warn_even_in_empty_documents_and_undo_clears_only_their_warnings() {
    let original = occurrence();
    let mut corrections = corrections(&original);
    let edit = corrections.edits[&original.id].clone();
    corrections.edits.insert("text-000099".into(), edit);
    for populated in [false, true] {
        let mut input = TextDocument {
            review_warnings: vec!["A reference file could not be matched".into()],
            ..TextDocument::default()
        };
        if populated {
            let mut other = occurrence();
            other.id = "text-000002".into();
            input.occurrences.push(other);
        }
        for _ in 0..2 {
            apply(&mut input, &corrections, 10.0).unwrap();
            assert_eq!(input.occurrences.len(), usize::from(populated));
            assert_eq!(input.review_warnings.len(), 3);
            assert_eq!(input.summary().flagged, 3);
            for id in corrections.edits.keys() {
                assert!(input.review_warnings.iter().any(|warning| {
                    warning.contains(id) && warning.contains("no matching occurrence")
                }));
            }
        }
        apply(&mut input, &TextCorrections::default(), 10.0).unwrap();
        assert_eq!(
            input.review_warnings,
            ["A reference file could not be matched"]
        );
        assert_eq!(input.summary().flagged, 1);
    }
}

#[test]
fn invalid_matching_edits_fail_without_changing_words_times_or_placement() {
    for invalid in 0..5 {
        let original = occurrence();
        let mut corrections = corrections(&original);
        let edit = corrections.edits.get_mut(&original.id).unwrap();
        match invalid {
            0 => edit.start_s = f64::NAN,
            1 => edit.end_s = 11.0,
            2 => edit.end_s = edit.start_s,
            3 => edit.presentation.font_size = Some(401.0),
            4 => {
                edit.presentation.anchor = Some(Point {
                    x: f64::INFINITY,
                    y: 0.0,
                })
            }
            _ => unreachable!(),
        }
        let mut input = document(original.clone());
        assert!(
            apply(&mut input, &corrections, 10.0).is_err(),
            "case {invalid}"
        );
        let text = &input.occurrences[0];
        assert!(!text.reviewed);
        assert_eq!(text.english, original.english);
        assert_eq!(
            (text.start_s, text.end_s),
            (original.start_s, original.end_s)
        );
        assert_eq!(text.frames, original.frames);
        assert_eq!(text.presentation, original.presentation);
    }
}
