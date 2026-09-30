use super::*;
use job_model::onscreen::{Point, TextKeyframe, TextPresentation, TextProvenance, TextTreatment};
use job_model::outputs::ShotCut;
use std::path::PathBuf;

fn rectangle([left, top, right, bottom]: [f64; 4]) -> Quad {
    let corner = |x, y| Point { x, y };
    Quad([
        corner(left, top),
        corner(right, top),
        corner(right, bottom),
        corner(left, bottom),
    ])
}

/// An occurrence seen at `box` over `times`, one frame between each pair, with its keyframe at
/// the start.
fn sighting(id: &str, japanese: &str, times: &[f64], pixels: [f64; 4]) -> TextOccurrence {
    let claude = found_by_claude(id);
    TextOccurrence {
        id: id.into(),
        start_s: times[0],
        end_s: times[times.len() - 1],
        japanese: japanese.into(),
        english: Some(format!("English of {japanese}")),
        confidence: 0.9,
        crops: vec![PathBuf::from(format!("visual/crops/{id}.png"))],
        frames: times
            .windows(2)
            .map(|pair| TextFrame {
                time_s: pair[0],
                end_s: pair[1],
                quad: rectangle(pixels),
                confidence: 0.9,
                surface_rgb: None,
            })
            .collect(),
        provenance: TextProvenance::default(),
        presentation: TextPresentation {
            treatment: if claude {
                TextTreatment::Nearby
            } else {
                TextTreatment::Auto
            },
            ..TextPresentation::default()
        },
        warnings: if claude {
            vec![FOUND_BY_CLAUDE.into()]
        } else {
            Vec::new()
        },
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: Some(TextKeyframe {
            time_s: times[0],
            image: format!("visual/keyframes/{id}.png").into(),
        }),
        ruby: Vec::new(),
    }
}

fn document(occurrences: Vec<TextOccurrence>) -> TextDocument {
    TextDocument {
        width: 1920,
        height: 1080,
        occurrences,
        ..TextDocument::default()
    }
}

fn find<'a>(document: &'a TextDocument, id: &str) -> &'a TextOccurrence {
    document
        .occurrences
        .iter()
        .find(|item| item.id == id)
        .unwrap_or_else(|| panic!("{id} is missing"))
}

fn tiles(item: &TextOccurrence) -> bool {
    item.frames.first().map(|frame| frame.time_s) == Some(item.start_s)
        && item.frames.last().map(|frame| frame.end_s) == Some(item.end_s)
        && item
            .frames
            .windows(2)
            .all(|pair| pair[1].time_s == pair[0].end_s)
}

const WANO: [f64; 4] = [747.0, 777.0, 1155.0, 870.0];
const KANJURO: [f64; 4] = [672.0, 885.0, 1250.0, 983.0];

/// Dressrosa 28 at 3:07: the name card is on screen from 187.58 to the cut at 192.75, but the
/// detector sees it from 188.67 and Claude adds its own boxes on two keyframes.
fn name_card() -> TextDocument {
    let mut wano = sighting(
        "text-000584",
        "ワノ国の侍",
        &[188.67, 189.0, 190.0, 191.0, 192.75],
        WANO,
    );
    wano.confidence = 0.95;
    let mut kanjuro = sighting(
        "text-000585",
        "夕立ち カン十郎",
        &[188.63, 190.0, 192.75],
        KANJURO,
    );
    kanjuro.english = None;
    kanjuro.confidence = 0.6;
    kanjuro
        .warnings
        .push("Translation needs review: the reading is uncertain.".into());
    let samurai = sighting(
        "text-000574",
        "サムライ",
        &[187.58, 188.79],
        [1062.0, 762.0, 1137.0, 783.0],
    );
    let mut rain = sighting(
        "text-000576-c3",
        "ゆうだち",
        &[187.71, 192.75],
        [680.0, 862.0, 800.0, 886.0],
    );
    rain.english = Some("yuudachi".into());
    rain.confidence = 0.7;
    document(vec![
        sighting(
            "text-000574-c2",
            "ワノ国の侍",
            &[187.58, 188.79],
            [634.0, 780.0, 1133.0, 853.0],
        ),
        sighting(
            "text-000574-c5",
            "夕立ちカン十郎",
            &[187.58, 188.79],
            [690.0, 880.0, 1240.0, 990.0],
        ),
        sighting(
            "text-000576-c1",
            "ワノ国の侍",
            &[187.71, 192.75],
            [557.0, 778.0, 1363.0, 848.0],
        ),
        sighting(
            "text-000576-c2",
            "夕立ち カン十郎",
            &[187.71, 192.75],
            [660.0, 890.0, 1260.0, 975.0],
        ),
        samurai,
        rain,
        wano,
        kanjuro,
    ])
}

fn card_cut() -> ShotChanges {
    ShotChanges {
        cuts: vec![ShotCut {
            time_s: 192.75,
            score: 60.0,
        }],
    }
}

#[test]
fn a_detector_sign_absorbs_claude_duplicates_into_one_tiled_span() {
    let mut card = name_card();
    unify(&mut card, &card_cut());
    let ids: Vec<_> = card
        .occurrences
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(ids, ["text-000584", "text-000585"]);
    let wano = find(&card, "text-000584");
    assert_eq!((wano.start_s, wano.end_s), (187.58, 192.75));
    assert!(tiles(wano), "{:?}", wano.frames);
    assert!(
        wano.frames
            .iter()
            .all(|frame| frame.quad == rectangle(WANO))
    );
    assert_eq!(wano.english.as_deref(), Some("English of ワノ国の侍"));
    assert_eq!(wano.presentation.treatment, TextTreatment::Auto);
    assert!(wano.warnings.is_empty(), "{:?}", wano.warnings);
    assert!(
        wano.provenance
            .reason
            .contains("Same writing as text-000576-c1, text-000574-c2.")
    );
    assert_eq!(wano.ruby, [rectangle([1062.0, 762.0, 1137.0, 783.0])]);
    assert_eq!(wano.crops.len(), 4);

    let kanjuro = find(&card, "text-000585");
    assert_eq!((kanjuro.start_s, kanjuro.end_s), (187.58, 192.75));
    assert!(tiles(kanjuro));
    assert!(
        kanjuro
            .frames
            .iter()
            .all(|frame| frame.quad == rectangle(KANJURO))
    );
    assert_eq!(kanjuro.japanese, "夕立ち カン十郎");
    assert_eq!(
        kanjuro.english.as_deref(),
        Some("English of 夕立ち カン十郎")
    );
    assert_eq!(kanjuro.confidence, 0.9);
    assert!(kanjuro.warnings.is_empty(), "{:?}", kanjuro.warnings);
    assert_eq!(kanjuro.ruby, [rectangle([680.0, 862.0, 800.0, 886.0])]);

    let once = card.clone();
    unify(&mut card, &card_cut());
    assert_eq!(card, once, "a second pass changes nothing");
}

#[test]
fn different_writing_in_the_same_place_stays_apart() {
    let mut input = document(vec![
        sighting("text-000001", "営業中", &[1.0, 2.0], WANO),
        sighting("text-000001-c1", "準備中", &[1.0, 2.0], WANO),
        sighting("text-000002", "営業", &[1.0, 2.0], KANJURO),
        sighting("text-000002-c1", "営", &[1.0, 2.0], KANJURO),
    ]);
    unify(&mut input, &ShotChanges::default());
    assert_eq!(input.occurrences.len(), 4);
}

#[test]
fn the_same_sign_joins_over_a_short_pause_but_never_across_a_cut() {
    let pieces = || {
        document(vec![
            sighting("text-000525", "幹部塔", &[157.5, 157.708333], WANO),
            sighting("text-000532", "幹部塔", &[157.8, 158.2], WANO),
        ])
    };
    let mut joined = pieces();
    unify(&mut joined, &ShotChanges::default());
    assert_eq!(joined.occurrences.len(), 1);
    assert!(tiles(&joined.occurrences[0]));
    assert_eq!(joined.occurrences[0].end_s, 158.2);

    let mut cut = pieces();
    let cuts = ShotChanges {
        cuts: vec![ShotCut {
            time_s: 157.75,
            score: 60.0,
        }],
    };
    unify(&mut cut, &cuts);
    assert_eq!(cut.occurrences.len(), 2);

    let mut far = document(vec![
        sighting("text-000525", "幹部塔", &[157.5, 157.708333], WANO),
        sighting("text-000540", "幹部塔", &[158.0, 158.5], WANO),
    ]);
    unify(&mut far, &ShotChanges::default());
    assert_eq!(far.occurrences.len(), 2);
}

#[test]
fn a_reading_that_differs_only_in_kana_size_or_spaces_is_the_same_writing() {
    assert!(same_writing(
        "工場長 キュイーン(20歳・女)",
        "キユイーン(20歳・女)"
    ));
    assert!(same_writing("夕立ち カン十郎", "夕立ちカン十郎"));
    assert!(!same_writing("ワノ国の侍", "侍"));
    assert!(!same_writing("SMILE", "SMILE"));
}

#[test]
fn the_same_writing_elsewhere_on_screen_stays_apart() {
    let mut input = document(vec![
        sighting(
            "text-000001",
            "幹部塔",
            &[1.0, 2.0],
            [100.0, 100.0, 300.0, 160.0],
        ),
        sighting(
            "text-000002",
            "幹部塔",
            &[1.0, 2.0],
            [900.0, 100.0, 1100.0, 160.0],
        ),
    ]);
    unify(&mut input, &ShotChanges::default());
    assert_eq!(input.occurrences.len(), 2);
}

#[test]
fn a_reviewed_occurrence_is_never_absorbed_and_keeps_the_owner_timing() {
    let mut reviewed = sighting(
        "text-000576-c1",
        "ワノ国の侍",
        &[187.71, 192.75],
        [557.0, 778.0, 1363.0, 848.0],
    );
    reviewed.reviewed = true;
    reviewed.english = Some("Samurai of Wano".into());
    let owner = reviewed.clone();
    let mut detector = sighting("text-000584", "ワノ国の侍", &[187.5, 192.75], WANO);
    detector.confidence = 0.99;
    let mut input = document(vec![detector.clone(), reviewed]);
    unify(&mut input, &ShotChanges::default());
    assert_eq!(input.occurrences.len(), 1);
    let kept = &input.occurrences[0];
    assert_eq!(kept.id, owner.id);
    assert_eq!((kept.start_s, kept.end_s), (owner.start_s, owner.end_s));
    assert_eq!(kept.frames, owner.frames);
    assert_eq!(kept.english, owner.english);
    assert_eq!(kept.presentation, owner.presentation);

    let mut other = owner.clone();
    other.id = "text-000576-c2".into();
    let mut both = document(vec![owner, other]);
    unify(&mut both, &ShotChanges::default());
    assert_eq!(both.occurrences.len(), 2);
}

#[test]
fn only_numbered_c_suffixes_mark_claude_found_writing() {
    assert!(found_by_claude("text-000574-c2"));
    assert!(found_by_claude("text-000574-c12"));
    assert!(!found_by_claude("text-000574"));
    assert!(!found_by_claude("text-000574-c"));
    assert!(!found_by_claude("sign-c"));
}

#[test]
#[ignore = "set TBD_VISUAL_TRANSLATE_FIXTURE to a job's visual/text_translate.json"]
fn a_translated_job_keeps_one_occurrence_per_sign() {
    let path = PathBuf::from(std::env::var_os("TBD_VISUAL_TRANSLATE_FIXTURE").expect("fixture"));
    let mut input: TextDocument = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let shots_path = path.parent().unwrap().parent().unwrap().join("shots.json");
    let shots: ShotChanges = serde_json::from_slice(&std::fs::read(shots_path).unwrap()).unwrap();
    let before = input.occurrences.len();
    unify(&mut input, &shots);
    println!(
        "{before} occurrences before, {} after",
        input.occurrences.len()
    );
    for item in input
        .occurrences
        .iter()
        .filter(|item| item.provenance.reason.contains("Same writing as") || !item.ruby.is_empty())
    {
        println!(
            "{} {:.3}..{:.3} {:?} {:?}, {} ruby; {}",
            item.id,
            item.start_s,
            item.end_s,
            item.japanese,
            item.english,
            item.ruby.len(),
            item.provenance
                .reason
                .split("Same writing as")
                .nth(1)
                .unwrap_or("")
        );
    }
    for (index, a) in input.occurrences.iter().enumerate() {
        for b in &input.occurrences[index + 1..] {
            assert!(same_sign(a, b, &shots).is_none(), "{} and {}", a.id, b.id);
        }
    }
}
