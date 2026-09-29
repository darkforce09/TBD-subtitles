use super::*;
use job_model::onscreen::{Point, TextFrame, TextKeyframe, TextPresentation, TextProvenance};
use std::path::PathBuf;

fn stream() -> VideoStream {
    VideoStream {
        index: 0,
        codec: "h264".into(),
        width: 1920,
        height: 1080,
        frame_rate_num: 24,
        frame_rate_den: 1,
        start_time_s: 0.0,
    }
}

fn rectangle(left: f64, top: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point {
            x: left + 300.0,
            y: top,
        },
        Point {
            x: left + 300.0,
            y: top + 60.0,
        },
        Point {
            x: left,
            y: top + 60.0,
        },
    ])
}

fn occurrence(quads: &[Quad], keyframe: Option<f64>) -> TextOccurrence {
    TextOccurrence {
        id: "text-000001".into(),
        start_s: 0.0,
        end_s: quads.len() as f64 * 0.5,
        japanese: "看板".into(),
        english: None,
        confidence: 0.9,
        crops: vec![PathBuf::from("visual/crops/text-000001.png")],
        frames: quads
            .iter()
            .enumerate()
            .map(|(i, &quad)| TextFrame {
                time_s: i as f64 * 0.5,
                end_s: (i + 1) as f64 * 0.5,
                quad,
                confidence: 0.9,
                surface_rgb: Some([240; 3]),
            })
            .collect(),
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: keyframe.map(|time_s| TextKeyframe {
            time_s,
            image: PathBuf::from("visual/keyframes/frame-00000012.png"),
        }),
    }
}

fn document(occurrences: Vec<TextOccurrence>) -> TextDocument {
    TextDocument {
        width: 1920,
        height: 1080,
        decoded_frames: 100,
        occurrences,
        proxy_width: 640,
        sample_step: 12,
        ..TextDocument::default()
    }
}

#[test]
fn a_static_surface_within_tolerance_takes_the_keyframe_quad_and_stays_auto() {
    let exact = rectangle(100.4, 200.2);
    let quads = [rectangle(99.0, 198.0), exact, rectangle(103.0, 201.0)];
    let mut document = document(vec![occurrence(&quads, Some(0.5))]);
    let calls = std::sync::Mutex::new(Vec::new());
    track(&mut document, &stream(), &|done, total| {
        calls.lock().unwrap().push((done, total))
    })
    .unwrap();
    let text = &document.occurrences[0];
    assert!(text.frames.iter().all(|frame| frame.quad == exact));
    assert_eq!(text.presentation.treatment, TextTreatment::Auto);
    assert!(text.warnings.is_empty());
    assert_eq!(calls.into_inner().unwrap(), [(1, 1)]);
    let times: Vec<_> = text.frames.iter().map(|f| (f.time_s, f.end_s)).collect();
    assert_eq!(
        times,
        [(0.0, 0.5), (0.5, 1.0), (1.0, 1.5)],
        "timing is unchanged"
    );
}

#[test]
fn a_moving_quad_flags_nearby_placement() {
    let quads = [
        rectangle(100.0, 200.0),
        rectangle(100.0, 200.0),
        rectangle(140.0, 200.0),
    ];
    let mut document = document(vec![occurrence(&quads, Some(0.0))]);
    track(&mut document, &stream(), &|_, _| {}).unwrap();
    let text = &document.occurrences[0];
    assert_eq!(text.presentation.treatment, TextTreatment::Nearby);
    assert_eq!(
        text.warnings,
        [
            "Tracking needs review: the writing moves between sampled frames. English uses nearby placement."
        ]
    );
    assert_eq!(
        text.frames[2].quad,
        rectangle(140.0, 200.0),
        "geometry is kept"
    );
}

#[test]
fn a_missing_or_unmatched_keyframe_flags_nearby_placement() {
    let quads = [rectangle(100.0, 200.0), rectangle(100.0, 200.0)];
    let mut document = document(vec![
        occurrence(&quads, None),
        occurrence(&quads, Some(0.25)),
        occurrence(&[], Some(0.0)),
    ]);
    track(&mut document, &stream(), &|_, _| {}).unwrap();
    for text in &document.occurrences {
        assert_eq!(text.presentation.treatment, TextTreatment::Nearby);
        assert_eq!(
            text.warnings,
            [
                "Tracking needs review: no keyframe geometry verifies the text surface. English uses nearby placement."
            ]
        );
    }
}

#[test]
fn the_tolerance_widens_with_the_screening_scale() {
    let mut scanned = document(Vec::new());
    assert!((tolerance(&scanned) - 6.0).abs() < 1e-9);
    scanned.proxy_width = 0;
    assert!((tolerance(&scanned) - 2.0).abs() < 1e-9);
    scanned.proxy_width = 3840;
    assert!((tolerance(&scanned) - 2.0).abs() < 1e-9);
    let quads = [rectangle(100.0, 200.0), rectangle(105.9, 200.0)];
    let mut document = document(vec![occurrence(&quads, Some(0.0))]);
    track(&mut document, &stream(), &|_, _| {}).unwrap();
    assert_eq!(
        document.occurrences[0].presentation.treatment,
        TextTreatment::Auto
    );
    assert_eq!(
        document.occurrences[0].frames[1].quad,
        rectangle(100.0, 200.0)
    );
}

#[test]
fn mismatched_dimensions_are_an_error_and_an_empty_document_passes() {
    let quads = [rectangle(100.0, 200.0)];
    let mut small = document(vec![occurrence(&quads, Some(0.0))]);
    small.width = 1280;
    assert!(track(&mut small, &stream(), &|_, _| {}).is_err());
    let mut empty = document(Vec::new());
    empty.width = 1280;
    assert!(track(&mut empty, &stream(), &|_, _| {}).is_ok());
}
