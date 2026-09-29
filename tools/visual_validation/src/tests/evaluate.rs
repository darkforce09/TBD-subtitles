//! Synthetic annotations exercise coverage, per-frame geometry and fail-closed reports.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use job_model::onscreen::{TextFrame, TextPresentation};

use super::*;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "tbd-visual-evaluate-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("fixture directory");
        Self { root }
    }

    fn write(&self, document: &TextDocument, expected: &Annotations) {
        pipeline::work_dir::write_json(&self.root.join("actual.json"), document)
            .expect("actual document");
        pipeline::work_dir::write_json(&self.root.join("annotations.json"), expected)
            .expect("annotations");
    }

    fn run(&self) -> (bool, Verdict) {
        let result = run(
            &self.root.join("actual.json"),
            &self.root.join("annotations.json"),
            &self.root.join("verdict.json"),
        );
        let verdict: Verdict = pipeline::work_dir::read_json(&self.root.join("verdict.json"))
            .expect("verdict even on failure");
        assert_eq!(result.is_ok(), verdict.passed);
        (result.is_ok(), verdict)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn quad(x: f64) -> Quad {
    Quad([
        Point { x, y: 100.0 },
        Point {
            x: x + 200.0,
            y: 100.0,
        },
        Point {
            x: x + 200.0,
            y: 200.0,
        },
        Point { x, y: 200.0 },
    ])
}

fn expected(label: &str, japanese: &str, x: f64) -> Expected {
    Expected {
        label: label.into(),
        japanese: japanese.into(),
        start_s: 1.0,
        end_s: 2.0,
        frames: [1.0, 1.5]
            .map(|time_s| Frame {
                time_s,
                quad: quad(x),
            })
            .into(),
    }
}

fn actual(expected: &Expected) -> TextOccurrence {
    TextOccurrence {
        source_fingerprint: None,
        id: expected.label.clone(),
        start_s: expected.start_s,
        end_s: expected.end_s,
        japanese: expected.japanese.clone(),
        english: Some("English translation".into()),
        confidence: 0.95,
        crops: vec![],
        frames: expected
            .frames
            .iter()
            .map(|frame| TextFrame {
                time_s: frame.time_s,
                end_s: frame.time_s + 1.0 / 24.0,
                quad: frame.quad,
                confidence: 0.98,
                surface_rgb: None,
            })
            .collect(),
        provenance: Default::default(),
        presentation: TextPresentation::default(),
        warnings: vec![],
        reviewed: false,
        rendered: Some(true),
        keyframe: None,
    }
}

fn sample() -> (TextDocument, Annotations) {
    let entry = expected("board", "ドレスローザ", 100.0);
    (
        TextDocument {
            review_warnings: Vec::new(),
            proxy_width: 0,
            sample_step: 0,
            width: 1920,
            height: 1080,
            decoded_frames: 48,
            occurrences: vec![actual(&entry)],
        },
        Annotations {
            fps: 24.0,
            height: 1080,
            occurrences: vec![entry],
        },
    )
}

fn check_files(document: &TextDocument, annotations: &Annotations) -> Verdict {
    let fixture = Fixture::new();
    fixture.write(document, annotations);
    fixture.run().1
}

fn failure(document: &TextDocument, annotations: &Annotations, message: &str) -> Verdict {
    let verdict = check_files(document, annotations);
    assert!(!verdict.passed, "unexpected pass: {message}");
    assert!(
        verdict
            .failures
            .iter()
            .any(|failure| failure.contains(message)),
        "expected {message:?}: {:?}",
        verdict.failures
    );
    verdict
}

#[test]
fn every_readable_occurrence_needs_its_own_rendered_translation_or_flag() {
    let (mut document, mut annotations) = sample();
    let title = expected("title", "運命の再会", 800.0);
    annotations.occurrences.push(title.clone());
    let verdict = failure(
        &document,
        &annotations,
        "title: readable occurrence missing",
    );
    assert_eq!(
        (verdict.readable, verdict.translated, verdict.flagged),
        (2, 1, 0)
    );
    document.occurrences.push(actual(&title));
    let verdict = check_files(&document, &annotations);
    assert!(verdict.passed);
    assert_eq!((verdict.translated, verdict.flagged), (2, 0));
}

#[test]
fn one_actual_track_cannot_satisfy_two_annotations() {
    let (document, mut annotations) = sample();
    let mut duplicate = annotations.occurrences[0].clone();
    duplicate.label = "another visible sign".into();
    annotations.occurrences.push(duplicate);
    let verdict = failure(&document, &annotations, "no distinct actual occurrence");
    assert_eq!(verdict.translated, 1);
}

#[test]
fn matching_reassigns_an_ambiguous_fallback_to_preserve_a_specific_translation() {
    let (mut document, mut annotations) = sample();
    annotations
        .occurrences
        .push(expected("name", "レベッカ", 100.0));
    let mut unreadable = actual(&annotations.occurrences[0]);
    unreadable.id = "unreadable".into();
    unreadable.japanese.clear();
    unreadable.english = None;
    unreadable.warnings.push("Reading uncertain".into());
    document.occurrences.insert(0, unreadable);
    let verdict = check_files(&document, &annotations);
    assert!(verdict.passed, "{:?}", verdict.failures);
    assert_eq!((verdict.translated, verdict.flagged), (1, 1));
}

#[test]
fn identical_simultaneous_signs_are_matched_by_geometry_in_any_order() {
    let (mut document, mut annotations) = sample();
    let other = expected("other-board", "ドレスローザ", 900.0);
    annotations.occurrences.push(other.clone());
    document.occurrences.insert(0, actual(&other));
    assert!(check_files(&document, &annotations).passed);
}

#[test]
fn rendering_failure_or_empty_english_never_counts_as_translated() {
    for rendered in [Some(false), None] {
        let (mut document, annotations) = sample();
        document.occurrences[0].rendered = rendered;
        assert_eq!(
            failure(&document, &annotations, "neither rendered English").translated,
            0
        );
        document.occurrences[0]
            .warnings
            .push("Rendering needs review".into());
        let verdict = check_files(&document, &annotations);
        assert!(verdict.passed);
        assert_eq!((verdict.translated, verdict.flagged), (0, 1));
    }
    let (mut document, annotations) = sample();
    document.occurrences[0].english = Some("  ".into());
    failure(&document, &annotations, "neither rendered English");
}

#[test]
fn timing_tolerance_uses_one_source_frame_for_both_edges_including_fallbacks() {
    let (mut document, mut annotations) = sample();
    annotations.fps = 60.0;
    document.occurrences[0].start_s += 1.0 / 60.0;
    document.occurrences[0].end_s += 1.0 / 60.0;
    assert!(check_files(&document, &annotations).passed);
    document.occurrences[0].end_s += 0.001;
    failure(&document, &annotations, "timing exceeds one source frame");
    document.occurrences[0].presentation.treatment = TextTreatment::Nearby;
    document.occurrences[0]
        .warnings
        .push("Tracking uncertain".into());
    failure(&document, &annotations, "timing exceeds one source frame");
}

#[test]
fn perspective_corner_error_requires_a_flagged_nearby_fallback() {
    let (mut document, annotations) = sample();
    document.occurrences[0].frames[1].quad.0[2].x += 6.0;
    failure(&document, &annotations, "tracking error 6.00px");
    document.occurrences[0].presentation.treatment = TextTreatment::Nearby;
    failure(&document, &annotations, "without flagged fallback");
    document.occurrences[0]
        .warnings
        .push("Perspective track unreliable".into());
    let verdict = check_files(&document, &annotations);
    assert!(verdict.passed);
    assert_eq!(verdict.flagged, 1);
    document.occurrences[0].reviewed = true;
    failure(&document, &annotations, "without flagged fallback");
}

#[test]
fn tracking_tolerance_scales_two_pixels_at_1080p_to_source_resolution() {
    let (mut document, mut annotations) = sample();
    document.height = 2160;
    annotations.height = 2160;
    document.occurrences[0].frames[1].quad.0[2].x += 4.0;
    assert!(check_files(&document, &annotations).passed);
    document.occurrences[0].frames[1].quad.0[2].x += 0.1;
    failure(&document, &annotations, "tracking error 2.05px");
}

#[test]
fn stale_sample_geometry_cannot_pass_even_when_the_occurrence_times_match() {
    let (mut document, annotations) = sample();
    document.occurrences[0].frames.pop();
    failure(&document, &annotations, "no observation covering");
}

#[test]
fn unreadable_occurrences_require_warning_geometry_and_correct_time_without_invented_english() {
    let (mut document, annotations) = sample();
    let item = &mut document.occurrences[0];
    item.japanese.clear();
    item.english = None;
    item.rendered = Some(false);
    failure(&document, &annotations, "readable occurrence missing");
    document.occurrences[0]
        .warnings
        .push("Unreadable crop".into());
    assert!(check_files(&document, &annotations).passed);
    let valid = document.clone();
    document.occurrences[0].frames[0].quad = quad(900.0);
    failure(&document, &annotations, "does not overlap");
    document = valid.clone();
    document.occurrences[0].end_s += 0.1;
    failure(&document, &annotations, "timing exceeds");
    document = valid;
    document.occurrences[0].english = Some("Invented reading".into());
    failure(&document, &annotations, "without invented English");
}

#[test]
fn perspective_bounding_box_overlap_does_not_count_as_text_surface_overlap() {
    let (mut document, mut annotations) = sample();
    let diagonal = Quad([
        Point { x: 100.0, y: 100.0 },
        Point { x: 110.0, y: 100.0 },
        Point { x: 210.0, y: 200.0 },
        Point { x: 200.0, y: 200.0 },
    ]);
    for frame in &mut annotations.occurrences[0].frames {
        frame.quad = diagonal;
    }
    let item = &mut document.occurrences[0];
    item.japanese.clear();
    item.english = None;
    item.warnings.push("Unreadable".into());
    for frame in &mut item.frames {
        frame.quad = diagonal;
        for point in &mut frame.quad.0 {
            point.y += 20.0;
        }
    }
    failure(&document, &annotations, "does not overlap");
}

#[test]
fn mismatched_source_edit_is_not_replaced_by_any_overlapping_warning() {
    let (mut document, annotations) = sample();
    document.occurrences[0].japanese = "レベッカ".into();
    document.occurrences[0]
        .warnings
        .push("Verify this reading".into());
    document.occurrences[0].presentation.treatment = TextTreatment::Nearby;
    failure(&document, &annotations, "source reading mismatched");
}

#[test]
fn invalid_annotations_fail_closed_with_a_written_verdict() {
    let (document, valid) = sample();
    let mut variants = Vec::new();
    let mut bad = valid.clone();
    bad.fps = 0.0;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.height = 0;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences.clear();
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].frames.clear();
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].japanese = " \n".into();
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].label.clear();
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences.push(bad.occurrences[0].clone());
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].start_s = -1.0;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].end_s = 0.5;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].frames[0].time_s = 5.0;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].frames[1].time_s = 1.0;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].frames[0].quad.0.swap(1, 2);
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].frames[0].quad.0[0].x = -1.0;
    variants.push(bad);
    for annotations in variants {
        let verdict = check_files(&document, &annotations);
        assert!(!verdict.passed);
        assert!(!verdict.failures.is_empty());
    }
}

#[test]
fn invalid_actual_tracks_and_duplicate_ids_fail_closed() {
    let (valid, annotations) = sample();
    let mut variants = Vec::new();
    let mut bad = valid.clone();
    bad.occurrences[0].frames.clear();
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].id.clear();
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences.push(bad.occurrences[0].clone());
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].end_s = 0.5;
    variants.push(bad);
    let mut bad = valid.clone();
    bad.occurrences[0].frames[0].quad.0.swap(1, 2);
    variants.push(bad);
    for document in variants {
        assert!(!check_files(&document, &annotations).passed);
    }
}

#[test]
fn malformed_input_overwrites_a_previous_passing_verdict() {
    let fixture = Fixture::new();
    let (document, annotations) = sample();
    fixture.write(&document, &annotations);
    assert!(fixture.run().0);
    fs::write(fixture.root.join("annotations.json"), b"{broken").expect("malformed annotations");
    let (passed, verdict) = fixture.run();
    assert!(!passed);
    assert!(verdict.failures[0].contains("invalid annotations"));
}
