use super::*;
use job_model::onscreen::{Point, TextFrame, TextPresentation, TextProvenance};
use std::path::PathBuf;

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

fn occurrence(id: &str, text: &str, quad: Quad) -> TextOccurrence {
    TextOccurrence {
        source_fingerprint: None,
        keyframe: None,
        ruby: Vec::new(),
        id: id.into(),
        start_s: 1.0,
        end_s: 2.0,
        japanese: text.into(),
        english: None,
        confidence: 0.98,
        crops: vec![PathBuf::from(format!("crops/{id}.png"))],
        frames: vec![
            TextFrame {
                time_s: 1.0,
                end_s: 1.5,
                quad,
                confidence: 0.99,
                surface_rgb: None,
            },
            TextFrame {
                time_s: 1.5,
                end_s: 2.0,
                quad,
                confidence: 0.99,
                surface_rgb: None,
            },
        ],
        provenance: TextProvenance {
            backend: "PP-OCRv5 / manga-ocr".into(),
            ..TextProvenance::default()
        },
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
    }
}

fn title() -> TextOccurrence {
    occurrence("title", "運命の再会", rectangle(365.0, 226.0, 912.0, 335.0))
}
fn document(occurrences: Vec<TextOccurrence>) -> TextDocument {
    TextDocument {
        review_warnings: Vec::new(),
        proxy_width: 0,
        sample_step: 0,
        width: 1301,
        height: 717,
        decoded_frames: 24,
        occurrences,
    }
}

fn timed(mut item: TextOccurrence, start: f64, count: usize, frame_s: f64) -> TextOccurrence {
    item.start_s = start;
    item.end_s = start + count as f64 * frame_s;
    let frame = item.frames[0].clone();
    item.frames = (0..count)
        .map(|index| TextFrame {
            time_s: start + index as f64 * frame_s,
            end_s: start + (index + 1) as f64 * frame_s,
            ..frame.clone()
        })
        .collect();
    item
}

#[test]
fn adjacent_equal_readings_keep_frames_crops_worst_confidence_and_provenance() {
    let first = timed(title(), 0.0, 2, 1.0 / 24.0);
    let mut second = timed(title(), first.end_s, 2, 1.0 / 24.0);
    second.id = "second".into();
    second.confidence = 0.79;
    second.crops.push("crops/second.png".into());
    second.provenance.reason = "Both local readers disagree on the small glyph.".into();
    second.warnings.push("Review reading".into());
    let expected_frames: Vec<_> = first.frames.iter().chain(&second.frames).cloned().collect();
    let mut input = document(vec![second, first]);
    consolidate_readings(&mut input, &ShotChanges::default());
    assert_eq!(input.occurrences.len(), 1);
    let item = &input.occurrences[0];
    assert_eq!(item.id, "title");
    assert_eq!(item.frames, expected_frames);
    assert_eq!(item.confidence, 0.79);
    assert_eq!(item.crops.len(), 2);
    assert_eq!(item.warnings, ["Review reading"]);
    assert!(
        item.provenance
            .reason
            .contains("Both local readers disagree")
    );
    assert!(
        item.provenance
            .reason
            .contains("Adjacent OCR evidence second")
    );
}

#[test]
fn one_frame_gap_can_join_but_larger_gaps_and_known_cuts_cannot() {
    for (gap, cut, expected) in [
        (1.0 / 24.0, false, 1),
        (2.0 / 24.0, false, 2),
        (0.0, true, 2),
    ] {
        let first = timed(title(), 0.0, 2, 1.0 / 24.0);
        let mut second = timed(title(), first.end_s + gap, 2, 1.0 / 24.0);
        second.id = "second".into();
        let cuts = ShotChanges {
            cuts: if cut {
                vec![job_model::outputs::ShotCut {
                    time_s: second.start_s,
                    score: 60.0,
                }]
            } else {
                Vec::new()
            },
        };
        let mut input = document(vec![first, second]);
        consolidate_readings(&mut input, &cuts);
        assert_eq!(input.occurrences.len(), expected, "gap {gap}, cut {cut}");
        if expected == 1 {
            assert_eq!(
                input.occurrences[0].frames.len(),
                4,
                "No invented gap frame"
            );
        }
    }
}

/// Frames between timestamps rounded to microseconds, as the detector records them.
fn rounded(mut item: TextOccurrence, times: &[f64]) -> TextOccurrence {
    let frame = item.frames[0].clone();
    item.frames = times
        .windows(2)
        .map(|pair| TextFrame {
            time_s: pair[0],
            end_s: pair[1],
            ..frame.clone()
        })
        .collect();
    item.start_s = times[0];
    item.end_s = times[times.len() - 1];
    item
}

#[test]
fn a_one_frame_gap_between_rounded_timestamps_still_joins_and_leaves_no_hole() {
    // A sign in Dressrosa 28: one sighting ends at 157.708333, the next starts at 157.75, and
    // the shortest rounded frame of the episode, at 256 s, sets the frame duration.
    let first = rounded(title(), &[157.625, 157.666667, 157.708333]);
    let mut second = rounded(title(), &[157.75, 157.791667, 157.833333]);
    second.id = "second".into();
    let mut later = rounded(
        occurrence("later", "別の看板", rectangle(10.0, 10.0, 200.0, 60.0)),
        &[256.041667, 256.083333],
    );
    later.crops.clear();
    let frame_s = 256.083333 - 256.041667;
    assert!(
        second.start_s - first.end_s > frame_s + 1e-6,
        "the gap exceeds one frame by rounding alone"
    );
    let mut input = document(vec![first, second, later]);
    consolidate_readings(&mut input, &ShotChanges::default());
    assert_eq!(input.occurrences.len(), 2);
    let item = &input.occurrences[0];
    assert_eq!((item.start_s, item.end_s), (157.625, 157.833333));
    assert_eq!(item.frames.len(), 4);
    assert!(
        item.frames
            .windows(2)
            .all(|pair| pair[1].time_s == pair[0].end_s),
        "the missing frame is covered by the previous sighting"
    );
}

#[test]
fn different_readings_regions_unknowns_and_overlapping_instances_stay_separate() {
    for variant in 0..5 {
        let mut first = timed(title(), 0.0, 2, 1.0 / 24.0);
        let mut second = timed(title(), first.end_s, 2, 1.0 / 24.0);
        second.id = "second".into();
        match variant {
            0 => second.japanese = "運命の別れ".into(),
            1 => {
                for frame in &mut second.frames {
                    frame.quad = rectangle(365.0, 380.0, 912.0, 489.0);
                }
            }
            2 => {
                first.japanese.clear();
                second.japanese.clear();
            }
            3 => second = timed(second, 0.0, 2, 1.0 / 24.0),
            _ => second.frames[0].time_s = f64::NAN,
        }
        let mut input = document(vec![first, second]);
        consolidate_readings(&mut input, &ShotChanges::default());
        assert_eq!(input.occurrences.len(), 2, "variant {variant}");
    }
}

#[test]
fn changed_text_between_equal_readings_prevents_rejoining() {
    let first = timed(title(), 0.0, 2, 1.0 / 24.0);
    let mut middle = timed(title(), first.end_s, 1, 1.0 / 24.0);
    middle.id = "middle".into();
    middle.japanese = "別れ".into();
    let mut last = timed(title(), middle.end_s, 2, 1.0 / 24.0);
    last.id = "last".into();
    let mut input = document(vec![first, middle, last]);
    consolidate_readings(&mut input, &ShotChanges::default());
    assert_eq!(input.occurrences.len(), 3);
}

#[test]
fn fragmented_namecard_consolidates_before_contained_ruby_grouping() {
    let mut base = timed(
        occurrence(
            "base",
            "コリーダコロシアム専属剣闘士",
            rectangle(476.0, 784.0, 1442.0, 859.0),
        ),
        0.0,
        8,
        1.0 / 24.0,
    );
    base.crops = vec!["crops/base.png".into()];
    let name = timed(
        occurrence("name", "レベッカ", rectangle(808.0, 888.0, 1112.0, 970.0)),
        0.0,
        4,
        1.0 / 24.0,
    );
    let continuation = timed(
        occurrence("name2", "レベッカ", rectangle(804.0, 864.0, 1118.0, 978.0)),
        name.end_s,
        4,
        1.0 / 24.0,
    );
    let ruby = timed(
        occurrence("ruby", "せん", rectangle(1092.0, 754.0, 1158.0, 784.0)),
        2.0 / 24.0,
        2,
        1.0 / 24.0,
    );
    let final_ruby = timed(
        occurrence("ruby2", "し", rectangle(1394.0, 760.0, 1416.0, 779.0)),
        4.0 / 24.0,
        3,
        1.0 / 24.0,
    );
    let mut input = document(vec![base, name, continuation, ruby, final_ruby]);
    consolidate_readings(&mut input, &ShotChanges::default());
    furigana::group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2);
    assert_eq!(input.occurrences[0].crops.len(), 3);
    assert_eq!(input.occurrences[1].frames.len(), 8);
}

#[test]
#[ignore = "set TBD_VISUAL_READ_FIXTURE to the annotated name-card job's `tbd-subtitles dump <job> outputs text_read`"]
fn annotated_namecard_pilot_has_two_base_lines_without_losing_unknowns() {
    let path = std::env::var_os("TBD_VISUAL_READ_FIXTURE").expect("fixture path");
    let mut input: TextDocument = dumped(path);
    let unknown_ids: Vec<_> = input
        .occurrences
        .iter()
        .filter(|item| item.start_s >= 5.25)
        .map(|item| item.id.clone())
        .collect();
    consolidate_readings(&mut input, &ShotChanges::default());
    furigana::group_furigana(&mut input);
    for item in &input.occurrences {
        println!(
            "{} {:.6}..{:.6} {:?}, {} frames, {} crops",
            item.id,
            item.start_s,
            item.end_s,
            item.japanese,
            item.frames.len(),
            item.crops.len()
        );
    }
    let bases: Vec<_> = input
        .occurrences
        .iter()
        .filter(|item| item.start_s < 5.25)
        .collect();
    assert_eq!(bases.len(), 2);
    assert_eq!(bases[0].japanese, "コリーダコロシアム専属剣闘士");
    assert_eq!(bases[1].japanese, "レベッカ");
    assert!(bases[0].provenance.reason.contains("Furigana evidence"));
    assert_eq!(
        unknown_ids,
        input
            .occurrences
            .iter()
            .filter(|item| item.start_s >= 5.25)
            .map(|item| item.id.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn cache_accepts_only_current_revision_and_finite_unit_confidence() {
    for confidence in [0.0, 0.5, 1.0] {
        let record = CachedReading {
            revision: READING_CACHE_REVISION,
            text: "作戦".into(),
            confidence,
        };
        assert_eq!(
            cached_reading(&serde_json::to_vec(&record).unwrap()),
            Some(("作戦".into(), confidence))
        );
    }
    for confidence in [-0.01, 1.01, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(!valid_confidence(confidence));
        let record = CachedReading {
            revision: READING_CACHE_REVISION,
            text: "作戦".into(),
            confidence,
        };
        assert!(cached_reading(&serde_json::to_vec(&record).unwrap()).is_none());
    }
    let old = CachedReading {
        revision: READING_CACHE_REVISION - 1,
        text: "作戦".into(),
        confidence: 0.9,
    };
    assert!(cached_reading(&serde_json::to_vec(&old).unwrap()).is_none());
    assert!(cached_reading(br#"["old tuple cache",0.9]"#).is_none());
    assert!(cached_reading(b"invalid JSON").is_none());
}

#[test]
fn cache_keys_separate_crop_content_and_decoder_revisions() {
    assert_eq!(
        reading_key(b"image", READING_CACHE_REVISION),
        reading_key(b"image", READING_CACHE_REVISION)
    );
    assert_ne!(
        reading_key(b"image", READING_CACHE_REVISION),
        reading_key(b"other image", READING_CACHE_REVISION)
    );
    assert_ne!(
        reading_key(b"image", READING_CACHE_REVISION),
        reading_key(b"image", READING_CACHE_REVISION + 1)
    );
}

/// The document of the row `tbd-subtitles dump <job> outputs <step>` printed to `path`.
fn dumped<T: serde::de::DeserializeOwned>(path: impl AsRef<Path>) -> T {
    let mut row: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    serde_json::from_value(row["value"].take()).unwrap()
}
