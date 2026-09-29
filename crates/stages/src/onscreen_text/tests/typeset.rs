use super::*;
use job_model::onscreen::{TextFrame, TextPresentation, TextProvenance};

fn rectangle(left: f64, top: f64, right: f64, bottom: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point { x: right, y: top },
        Point {
            x: right,
            y: bottom,
        },
        Point { x: left, y: bottom },
    ])
}

fn occurrence() -> TextOccurrence {
    TextOccurrence {
        source_fingerprint: None,
        id: "sign-1".into(),
        start_s: 1.0,
        end_s: 2.0,
        japanese: "作戦".into(),
        english: Some("OPERATION".into()),
        confidence: 0.99,
        crops: Vec::new(),
        frames: vec![
            TextFrame {
                time_s: 1.0,
                end_s: 1.5,
                quad: rectangle(200.0, 100.0, 800.0, 260.0),
                confidence: 0.99,
                surface_rgb: Some([210, 200, 180]),
            },
            TextFrame {
                time_s: 1.5,
                end_s: 2.0,
                quad: rectangle(200.0, 100.0, 800.0, 260.0),
                confidence: 0.99,
                surface_rgb: Some([210, 200, 180]),
            },
        ],
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
    }
}

fn document(text: TextOccurrence) -> TextDocument {
    TextDocument {
        review_warnings: Vec::new(),
        width: 1920,
        height: 1080,
        decoded_frames: 48,
        occurrences: vec![text],
    }
}

#[test]
fn safe_automatic_replacement_merges_identical_frames_and_preserves_a_stable_palette() {
    let mut input = occurrence();
    input.frames[1].surface_rgb = Some([212, 199, 181]);
    let mut input = document(input);
    let output = events(&mut input).unwrap();
    assert_eq!(input.occurrences[0].rendered, Some(true));
    if Font::load().is_ok() {
        assert_eq!(
            input.occurrences[0].presentation.treatment,
            TextTreatment::Replace
        );
        assert_eq!(output.lines().count(), 2);
        assert!(output.contains("Dialogue: 10,0:00:01.00,0:00:02.00"));
        assert!(output.contains("\\1c&HB4C8D2&"));
        assert!(!output.contains("\\1c&HB5C7D4&"));
    } else {
        assert_eq!(
            input.occurrences[0].presentation.treatment,
            TextTreatment::Nearby
        );
        assert!(
            input.occurrences[0]
                .warnings
                .iter()
                .any(|w| w.contains("font"))
        );
    }
}

#[test]
fn texture_occlusion_and_forced_replacement_never_authorize_a_mask() {
    for tracking_warning in [false, true] {
        let mut text = occurrence();
        text.presentation.treatment = TextTreatment::Replace;
        if tracking_warning {
            text.warnings
                .push("Tracking needs review: occlusion obscures the surface.".into());
        } else {
            text.frames[1].surface_rgb = None;
        }
        let mut input = document(text);
        let output = events(&mut input).unwrap();
        assert_eq!(
            input.occurrences[0].presentation.treatment,
            TextTreatment::Nearby
        );
        assert_eq!(input.occurrences[0].rendered, Some(true));
        assert!(!output.contains("Dialogue: 10,"));
        assert!(output.contains("Dialogue: 21,"));
        assert!(!input.occurrences[0].warnings.is_empty());
    }
}

#[test]
fn lower_namecards_leave_the_dialogue_band_clear() {
    let mut text = occurrence();
    for frame in &mut text.frames {
        frame.quad = rectangle(200.0, 760.0, 1500.0, 980.0);
    }
    let mut input = document(text);
    let output = events(&mut input).unwrap();
    assert_eq!(
        input.occurrences[0].presentation.treatment,
        TextTreatment::Nearby
    );
    assert!(!output.contains("Dialogue: 10,"));
    assert!(output.contains("\\pos("));
}

#[test]
fn crowded_scene_is_flagged_without_failing_or_drawing_over_other_labels() {
    let mut input = document(occurrence());
    input.occurrences = (0..60)
        .map(|index| {
            let mut text = occurrence();
            text.id = format!("credit-{index}");
            text.presentation.treatment = TextTreatment::Nearby;
            text.english = Some(format!(
                "Credit {index}: {}",
                "A long production credit requiring careful readable placement. ".repeat(4)
            ));
            text
        })
        .collect();
    let output = events(&mut input).unwrap();
    assert!(
        input
            .occurrences
            .iter()
            .any(|text| text.rendered == Some(false))
    );
    for text in input
        .occurrences
        .iter()
        .filter(|text| text.rendered == Some(false))
    {
        assert!(text.english.is_some());
        assert!(
            text.warnings
                .iter()
                .any(|w| w.starts_with("No rendered translation:"))
        );
    }
    let rects: Vec<_> = output
        .lines()
        .filter(|line| line.starts_with("Dialogue: 20,"))
        .map(|line| {
            let path = line.split("}m ").nth(1).unwrap();
            let numbers: Vec<f64> = path
                .split_whitespace()
                .filter_map(|n| n.parse().ok())
                .collect();
            (numbers[0], numbers[1], numbers[2], numbers[5])
        })
        .collect();
    for (index, rect) in rects.iter().enumerate() {
        assert!(rect.3 <= TEXT_BOTTOM);
        for other in &rects[index + 1..] {
            assert!(!overlaps(*rect, *other));
        }
    }
    assert_eq!(
        input.summary().unresolved,
        input
            .occurrences
            .iter()
            .filter(|t| t.rendered == Some(false))
            .count()
    );
}

#[test]
fn correcting_crowded_timing_can_render_without_a_stale_unrendered_warning() {
    let mut text = occurrence();
    text.presentation.treatment = TextTreatment::Nearby;
    text.rendered = Some(false);
    text.warnings
        .push("No rendered translation: crowded scene.".into());
    text.start_s = 3.0;
    text.end_s = 4.0;
    let mut input = document(text);
    assert!(!events(&mut input).unwrap().is_empty());
    assert_eq!(input.occurrences[0].rendered, Some(true));
    assert!(
        !input.occurrences[0]
            .warnings
            .iter()
            .any(|w| w.starts_with("No rendered translation:"))
    );
}

#[test]
fn subtitle_content_cannot_inject_ass_commands_or_event_lines() {
    let mut text = occurrence();
    text.presentation.treatment = TextTreatment::Nearby;
    text.english = Some("{\\pos(0,0)} literal \\N\nDialogue: 99,malicious".into());
    let mut input = document(text);
    let output = events(&mut input).unwrap();
    assert_eq!(output.lines().count(), 2);
    assert!(!output.contains("{\\pos(0,0)}"));
    assert!(output.contains("\\{\\\u{2060}pos(0,0)\\}"));
    assert!(output.contains("literal \\\u{2060}N\\NDialogue:"));
}

#[test]
fn moving_frames_keep_their_own_placement_and_presentation_timing() {
    let Some(font) = Font::load().ok() else {
        return;
    };
    let mut text = occurrence();
    text.start_s = 1.234;
    text.end_s = 1.334;
    text.frames[0].time_s = 1.234;
    text.frames[0].end_s = 1.284;
    text.frames[1].time_s = 1.284;
    text.frames[1].end_s = 1.334;
    text.frames[1].quad = rectangle(220.0, 100.0, 820.0, 260.0);
    let result = replacement(&text, text.english.as_ref().unwrap(), &font, 1.0, 1.0).unwrap();
    assert_eq!(result.len(), 4);
    assert_eq!((result[0].start, result[0].end), (123, 128));
    assert_eq!((result[2].start, result[2].end), (128, 133));
    assert!(result[1].text.contains("\\pos(500.00,180.00)"));
    assert!(result[3].text.contains("\\pos(520.00,180.00)"));
}

#[test]
fn edited_size_position_and_timing_are_used_without_leaving_the_safe_surface() {
    let Some(font) = Font::load().ok() else {
        return;
    };
    let mut text = occurrence();
    text.start_s = 1.25;
    text.end_s = 1.75;
    text.presentation.font_size = Some(24.0);
    text.presentation.anchor = Some(Point { x: 550.0, y: 180.0 });
    let result = replacement(&text, text.english.as_ref().unwrap(), &font, 1.0, 1.0).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!((result[0].start, result[0].end), (125, 175));
    assert!(result[1].text.contains("\\pos(550.00,180.00)"));
    assert!(result[1].text.contains("\\fs24.00"));
    text.presentation.anchor = Some(Point {
        x: 1700.0,
        y: 500.0,
    });
    assert!(replacement(&text, text.english.as_ref().unwrap(), &font, 1.0, 1.0).is_err());
}

#[test]
fn timing_extended_beyond_observed_geometry_uses_readable_nearby_text() {
    let mut text = occurrence();
    text.start_s = 0.5;
    text.end_s = 2.5;
    let mut input = document(text);
    let output = events(&mut input).unwrap();
    assert!(output.contains("0:00:00.50,0:00:02.50"));
    assert_eq!(
        input.occurrences[0].presentation.treatment,
        TextTreatment::Nearby
    );
}

#[test]
fn perspective_glyphs_are_portable_paths_and_missing_glyphs_trigger_fallback() {
    let Some(font) = Font::load().ok() else {
        return;
    };
    let mut text = occurrence();
    for frame in &mut text.frames {
        frame.quad = Quad([
            Point { x: 200.0, y: 100.0 },
            Point { x: 800.0, y: 120.0 },
            Point { x: 750.0, y: 280.0 },
            Point { x: 220.0, y: 250.0 },
        ]);
    }
    let result = replacement(&text, text.english.as_ref().unwrap(), &font, 1.0, 1.0).unwrap();
    assert_eq!(result.len(), 2);
    assert!(result[1].text.contains("\\p4"));
    assert!(!result[1].text.contains("OPERATION"));
    assert!(result[1].text.matches(" l ").count() > 20);
    text.english = Some("Unknown \u{10ffff}".into());
    let mut input = document(text);
    events(&mut input).unwrap();
    assert_eq!(
        input.occurrences[0].presentation.treatment,
        TextTreatment::Nearby
    );
    assert!(
        input.occurrences[0]
            .warnings
            .iter()
            .any(|w| w.contains("glyph"))
    );
}

#[test]
fn unreadable_writing_never_gets_invented_english_or_a_rendered_status() {
    let mut text = occurrence();
    text.english = None;
    let mut input = document(text);
    assert!(events(&mut input).unwrap().is_empty());
    assert_eq!(input.occurrences[0].rendered, Some(false));
    assert_eq!(input.summary().unresolved, 1);
    assert_eq!(input.summary().flagged, 1);
}

#[test]
fn uncertain_candidates_never_render_until_the_owner_reviews_them() {
    for confidence in [
        0.3,
        0.5,
        0.849,
        -1.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        let mut text = occurrence();
        text.confidence = confidence;
        text.english = Some("False positive candidate".into());
        text.presentation.treatment = TextTreatment::Nearby;
        let mut input = document(text);
        for _ in 0..2 {
            assert!(events(&mut input).unwrap().is_empty());
            assert_eq!(
                input.occurrences[0].english.as_deref(),
                Some("False positive candidate")
            );
            assert_eq!(input.occurrences[0].rendered, Some(false));
            assert_eq!(input.summary().unresolved, 1);
            assert_eq!(input.summary().flagged, 1);
            assert_eq!(
                input.occurrences[0]
                    .warnings
                    .iter()
                    .filter(|warning| warning.starts_with("No rendered translation:"))
                    .count(),
                1
            );
        }
        input.occurrences[0].english = Some("Owner verified wording".into());
        input.occurrences[0].reviewed = true;
        assert!(!events(&mut input).unwrap().is_empty());
        assert_eq!(input.occurrences[0].rendered, Some(true));
        assert_eq!(input.summary().unresolved, 0);
        assert!(
            !input.occurrences[0]
                .warnings
                .iter()
                .any(|warning| warning.starts_with("No rendered translation:"))
        );
    }
    let mut text = occurrence();
    text.confidence = 0.85;
    text.presentation.treatment = TextTreatment::Nearby;
    let mut input = document(text);
    assert!(!events(&mut input).unwrap().is_empty());
    assert_eq!(input.occurrences[0].rendered, Some(true));
}

#[test]
fn long_words_wrap_with_every_character_preserved() {
    let text = "abcdefghijklmnopqrstuvwxyz".repeat(5);
    let output = wrap(&text, 36.0, 120.0, None);
    assert_eq!(output.replace('\n', ""), text);
    assert!(
        output
            .lines()
            .all(|line| measure(line, 36.0, None) <= 120.0)
    );
}
