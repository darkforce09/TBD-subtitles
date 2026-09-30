use super::*;
use job_model::onscreen::{Point, TextFrame, TextKeyframe, TextPresentation, TextProvenance};
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

/// One frame over `start_s..end_s`, as Claude-found writing and single sightings carry.
fn spanning(id: &str, text: &str, (start_s, end_s): (f64, f64), quad: Quad) -> TextOccurrence {
    let mut item = occurrence(id, text, quad);
    item.start_s = start_s;
    item.end_s = end_s;
    item.frames.truncate(1);
    item.frames[0].time_s = start_s;
    item.frames[0].end_s = end_s;
    item
}

fn title() -> TextOccurrence {
    occurrence("title", "運命の再会", rectangle(365.0, 226.0, 912.0, 335.0))
}
fn ruby() -> TextOccurrence {
    occurrence("ruby", "うん", rectangle(381.0, 181.0, 466.0, 230.0))
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

#[test]
fn title_ruby_becomes_evidence_without_changing_its_base_line() {
    let base = title();
    let parent_frames = base.frames.clone();
    let mut input = document(vec![ruby(), base]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 1);
    let parent = &input.occurrences[0];
    assert_eq!(parent.japanese, "運命の再会");
    assert_eq!(parent.frames, parent_frames);
    assert_eq!(parent.confidence, 0.98);
    assert_eq!(
        parent.crops,
        vec![
            PathBuf::from("crops/title.png"),
            PathBuf::from("crops/ruby.png")
        ]
    );
    assert_eq!(parent.ruby, [rectangle(381.0, 181.0, 466.0, 230.0)]);
    assert!(parent.provenance.reason.contains("Furigana evidence ruby"));
    assert!(parent.provenance.reason.contains("うん"));
    assert!(parent.warnings.is_empty());
    let once = input.clone();
    group_furigana(&mut input);
    assert_eq!(input, once);
}

#[test]
fn namecard_ruby_can_be_uncertain_without_replacing_confident_kanji() {
    let base = occurrence(
        "name-title",
        "コリーダコロシアム専属剣闘士",
        rectangle(411.0, 676.0, 1241.0, 739.0),
    );
    let mut ruby = occurrence(
        "name-ruby",
        "せんでくけんどうし",
        rectangle(940.0, 647.0, 1224.0, 676.0),
    );
    ruby.confidence = 0.79;
    ruby.warnings
        .push("The local readers could not agree on a confident reading.".into());
    let mut input = document(vec![base, ruby]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 1);
    assert_eq!(
        input.occurrences[0].japanese,
        "コリーダコロシアム専属剣闘士"
    );
    assert!(
        input.occurrences[0]
            .provenance
            .reason
            .contains("0.79 confidence")
    );
    assert!(input.occurrences[0].warnings.is_empty());
}

#[test]
fn separate_furigana_fragments_merge_into_one_base_without_dropping_other_lines() {
    let mut second = ruby();
    second.id = "ruby2".into();
    second.japanese = "めい".into();
    second.crops = vec![PathBuf::from("crops/ruby2.png")];
    for frame in &mut second.frames {
        frame.quad = rectangle(492.0, 184.0, 573.0, 228.0);
    }
    let independent = occurrence(
        "subtitle",
        "ハイエナのベラミー",
        rectangle(190.0, 380.0, 1100.0, 490.0),
    );
    let mut input = document(vec![title(), ruby(), second, independent]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2);
    assert_eq!(input.occurrences[0].crops.len(), 3);
    assert_eq!(input.occurrences[0].ruby.len(), 2);
    assert_eq!(input.occurrences[1].id, "subtitle");
}

#[test]
fn similarly_positioned_text_is_not_grouped_without_kana_and_kanji_evidence() {
    for (reading, parent) in [
        ("SOP", "運命の再会"),
        ("再会", "運命の再会"),
        ("...", "運命の再会"),
        ("うん", "ベラミー"),
    ] {
        let mut child = ruby();
        child.japanese = reading.into();
        let mut base = title();
        base.japanese = parent.into();
        let mut input = document(vec![child, base]);
        group_furigana(&mut input);
        assert_eq!(input.occurrences.len(), 2, "{reading}/{parent}");
    }
}

#[test]
fn independent_small_labels_away_from_the_base_or_outside_its_timing_survive() {
    for quad in [
        rectangle(381.0, 120.0, 466.0, 169.0),
        rectangle(920.0, 181.0, 1005.0, 230.0),
        rectangle(381.0, 340.0, 466.0, 389.0),
    ] {
        let mut child = ruby();
        for frame in &mut child.frames {
            frame.quad = quad;
        }
        let mut input = document(vec![title(), child]);
        group_furigana(&mut input);
        assert_eq!(input.occurrences.len(), 2);
    }
    let mut child = ruby();
    child.start_s = 2.0;
    child.end_s = 2.5;
    child.frames.remove(0);
    child.frames[0].time_s = 2.0;
    child.frames[0].end_s = 2.5;
    let mut input = document(vec![title(), child]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2);
}

#[test]
fn ruby_may_start_later_and_finish_earlier_with_all_frames_contained() {
    let mut base = title();
    base.end_s = 2.5;
    base.frames.push(TextFrame {
        time_s: 2.0,
        end_s: 2.5,
        ..base.frames[1].clone()
    });
    let mut child = ruby();
    child.start_s = 1.5;
    child.frames.remove(0);
    let mut input = document(vec![base, child]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 1);
    assert_eq!(
        (input.occurrences[0].start_s, input.occurrences[0].end_s),
        (1.0, 2.5)
    );
    assert!(
        input.occurrences[0]
            .provenance
            .reason
            .contains("Furigana evidence ruby")
    );
}

#[test]
fn ruby_outlasting_its_base_by_less_than_half_its_span_still_folds() {
    let mut child = spanning(
        "ruby",
        "うん",
        (1.2, 2.6),
        rectangle(381.0, 181.0, 466.0, 230.0),
    );
    let mut input = document(vec![title(), child.clone()]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 1);
    child.start_s = 1.8;
    child.frames[0].time_s = 1.8;
    let mut input = document(vec![title(), child]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2, "only a quarter shares the base");
}

#[test]
fn contained_ruby_uses_contemporary_geometry_and_rejects_parent_gaps() {
    let mut base = title();
    base.frames[0].quad = rectangle(1.0, 20.0, 540.0, 129.0);
    let mut child = ruby();
    child.start_s = 1.5;
    child.frames.remove(0);
    let mut input = document(vec![base, child]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 1);
    let mut base = title();
    base.frames[0].end_s = 1.4;
    let mut input = document(vec![base, ruby()]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2);
}

#[test]
fn moving_apart_mid_scene_or_ambiguous_overlapping_bases_prevent_grouping() {
    let mut child = ruby();
    child.frames[1].quad = rectangle(1000.0, 181.0, 1085.0, 230.0);
    let mut input = document(vec![title(), child]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2);
    let mut other = title();
    other.id = "ambiguous-base".into();
    let mut input = document(vec![title(), other, ruby()]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 3);
}

#[test]
fn a_base_box_that_briefly_covers_part_of_the_line_does_not_break_the_pair() {
    // Dressrosa 28 at 17:19: the detector box of the caption covers only the name for a moment.
    let mut base = occurrence(
        "text-002012",
        "工場長 キュイーン(20歳・女)",
        rectangle(480.0, 891.0, 1443.0, 972.0),
    );
    base.end_s = 4.0;
    base.frames = (0..6)
        .map(|index| TextFrame {
            time_s: 1.0 + 0.5 * f64::from(index),
            end_s: 1.5 + 0.5 * f64::from(index),
            ..base.frames[0].clone()
        })
        .collect();
    base.frames[2].quad = rectangle(738.0, 890.0, 1440.0, 976.0);
    let mut reading = occurrence("text-002011", "こう", rectangle(492.0, 868.0, 544.0, 897.0));
    reading.end_s = 4.0;
    reading.frames = base
        .frames
        .iter()
        .map(|frame| TextFrame {
            quad: rectangle(492.0, 868.0, 544.0, 897.0),
            ..frame.clone()
        })
        .collect();
    let mut input = document(vec![base, reading]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 1);
    assert_eq!(input.occurrences[0].ruby.len(), 1);
}

#[test]
fn vertical_signs_and_tall_single_kana_regions_remain_independent() {
    let base = occurrence("vertical", "運命", rectangle(380.0, 226.0, 480.0, 600.0));
    let mut input = document(vec![base, ruby()]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2);
    let mut child = ruby();
    for frame in &mut child.frames {
        frame.quad = rectangle(400.0, 150.0, 430.0, 224.0);
    }
    let mut input = document(vec![title(), child]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2);
}

#[test]
fn wide_ruby_touching_a_short_kanji_word_folds_and_records_its_keyframe_box() {
    // Dressrosa 28: おうきゅう is wider than 王宮 and its bottom runs into the kanji.
    let mut palace = occurrence(
        "text-000769",
        "王宮",
        rectangle(1613.0, 140.0, 1745.0, 211.0),
    );
    palace.english = Some("Royal Palace".into());
    palace.keyframe = Some(TextKeyframe {
        time_s: 1.75,
        image: "visual/keyframes/palace.png".into(),
    });
    let mut reading = occurrence(
        "text-000768",
        "おうきゅう",
        rectangle(1623.0, 114.0, 1741.0, 146.0),
    );
    reading.english = Some("oukyuu".into());
    reading.frames[0].quad = rectangle(1622.0, 113.0, 1740.0, 145.0);
    let mut input = document(vec![reading, palace]);
    input.width = 1920;
    input.height = 1080;
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 1);
    let palace = &input.occurrences[0];
    assert_eq!(palace.english.as_deref(), Some("Royal Palace"));
    assert_eq!(palace.ruby, [rectangle(1623.0, 114.0, 1741.0, 146.0)]);
    assert!(
        palace
            .provenance
            .reason
            .contains("Furigana evidence text-000768")
    );
}

#[test]
fn claude_found_ruby_over_a_claude_box_folds_whatever_its_english() {
    // Dressrosa 28 at 3:07: サムライ above the box Claude gave ワノ国の侍.
    let mut line = spanning(
        "text-000574-c2",
        "ワノ国の侍",
        (187.58, 188.79),
        rectangle(634.0, 780.0, 1133.0, 853.0),
    );
    line.english = Some("Samurai of Wano Country".into());
    let mut reading = spanning(
        "text-000574",
        "サムライ",
        (187.58, 188.79),
        rectangle(1062.0, 762.0, 1137.0, 783.0),
    );
    reading.english = Some("Samurai".into());
    let mut kuni = spanning(
        "text-000574-c3",
        "くに",
        (187.58, 188.79),
        rectangle(800.0, 760.0, 850.0, 782.0),
    );
    kuni.english = Some("kuni".into());
    kuni.confidence = 0.6;
    let mut input = document(vec![line, reading, kuni]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 1);
    assert_eq!(input.occurrences[0].ruby.len(), 2);
}

#[test]
fn a_standalone_kana_line_far_from_any_kanji_stays() {
    let far = occurrence("far", "ありがとう", rectangle(100.0, 600.0, 400.0, 640.0));
    let mut input = document(vec![title(), far]);
    group_furigana(&mut input);
    assert_eq!(input.occurrences.len(), 2);
    assert!(input.occurrences.iter().all(|item| item.ruby.is_empty()));
}

#[test]
fn near_identical_ruby_boxes_are_recorded_once() {
    let mut base = title();
    add_ruby(&mut base, rectangle(381.0, 181.0, 466.0, 230.0));
    add_ruby(&mut base, rectangle(382.0, 181.0, 467.0, 231.0));
    add_ruby(&mut base, rectangle(492.0, 184.0, 573.0, 228.0));
    assert_eq!(base.ruby.len(), 2);
}
