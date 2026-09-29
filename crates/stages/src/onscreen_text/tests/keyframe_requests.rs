use super::super::tests::{
    K1, K2, Temporary, build_requests, found, frame_document, rectangle, region, sighting,
};
use super::*;
use job_model::onscreen::{TextCorrections, TextKeyframe};
use serde_json::Value;

fn hinted(index: usize, id: &str, bbox: [f64; 4]) -> Region {
    Region {
        index,
        id: id.into(),
        crop: PathBuf::from(format!("/job/visual/crops/{id}.png")),
        bbox,
        ocr_reading: "作戦".into(),
        reading_confidence: 0.9,
    }
}

/// The request for two signs on k1: `a` at the top left, `b` in the middle.
fn two_regions() -> Request {
    Request {
        keyframe: Path::new("/job").join(K1),
        regions: vec![
            hinted(0, "a", [0.1, 0.1, 0.3, 0.2]),
            hinted(1, "b", [0.5, 0.5, 0.7, 0.6]),
        ],
        prompt: "Read the signs.".into(),
        retry_generation: 0,
    }
}

fn two_answers() -> KeyframeAnswer {
    KeyframeAnswer {
        regions: vec![
            region("r1", "作戦", Some("Operation"), 0.95, [0.1, 0.1, 0.3, 0.2]),
            RegionAnswer {
                reason: "Only a painted pattern.".into(),
                ..region("r2", "", None, 0.9, [0.5, 0.5, 0.7, 0.6])
            },
        ],
        other_text: Vec::new(),
    }
}

/// The document `two_regions` was built from.
fn two_signs() -> TextDocument {
    frame_document(vec![
        sighting("a", (4.0, 5.0), "作單", K1, [192.0, 108.0, 576.0, 216.0]),
        sighting("b", (3.0, 6.0), "模様", K1, [960.0, 540.0, 1344.0, 648.0]),
    ])
}

fn keys(value: &Value) -> Vec<&str> {
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

/// Every object in the schema is closed and requires every property it names.
fn assert_closed(schema: &Value) {
    match schema["type"].as_str() {
        Some("object") => {
            assert_eq!(schema["additionalProperties"], false, "{schema}");
            let mut required = schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .map(|name| name.as_str().unwrap())
                .collect::<Vec<_>>();
            required.sort_unstable();
            assert_eq!(keys(&schema["properties"]), required, "{schema}");
            schema["properties"]
                .as_object()
                .unwrap()
                .values()
                .for_each(assert_closed);
        }
        Some("array") => assert_closed(&schema["items"]),
        _ => {}
    }
}

#[test]
fn occurrences_on_one_keyframe_share_one_request_and_one_dialogue_context() {
    let temp = Temporary::new();
    let mut unframed = sighting("d", (2.0, 3.0), "港", K1, [0.0, 0.0, 192.0, 108.0]);
    unframed.keyframe = None;
    let mut uncropped = sighting("e", (2.0, 3.0), "港", K1, [0.0, 0.0, 192.0, 108.0]);
    uncropped.crops.clear();
    let document = frame_document(vec![
        sighting("a", (4.0, 5.0), "作戦", K1, [192.0, 108.0, 576.0, 216.0]),
        unframed,
        sighting("b", (3.0, 5.0), "海賊", K1, [960.0, 540.0, 1344.0, 648.0]),
        sighting("c", (25.0, 26.0), "港", K2, [192.0, 756.0, 576.0, 864.0]),
        uncropped,
    ]);
    let corrections = TextCorrections {
        retry: vec!["b".into(), "b".into(), "a".into()],
        ..TextCorrections::default()
    };
    let requests = build_requests(
        &temp.0,
        &document,
        &corrections,
        &["Doflamingo".to_string()],
    );
    let [shared, single] = &requests[..] else {
        panic!("two requests expected: {requests:?}")
    };
    assert_eq!(shared.keyframe, temp.0.join(K1));
    let listed = shared
        .regions
        .iter()
        .map(|region| (region.index, region.id.as_str(), region.crop.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        listed,
        [
            (0, "a", temp.0.join("visual/crops/a.png")),
            (2, "b", temp.0.join("visual/crops/b.png")),
        ]
    );
    assert_eq!(shared.retry_generation, 2);
    assert!(shared.prompt.starts_with("{\"images\":"));
    assert_eq!(
        serde_json::from_str::<Value>(&shared.prompt).unwrap(),
        json!({
            "images": "image 1 is the whole frame at reduced size; images 2..3 are the full-resolution crops of regions r1..r2 in that order",
            "regions": [
                {"id": "r1", "image": 2, "bbox": [0.1, 0.1, 0.3, 0.2], "ocr_reading": "作戦", "reading_confidence": 0.9},
                {"id": "r2", "image": 3, "bbox": [0.5, 0.5, 0.7, 0.6], "ocr_reading": "海賊", "reading_confidence": 0.9},
            ],
            "nearby_dialogue": "The operation will begin shortly.",
            "glossary": ["Doflamingo"],
        })
    );
    assert_eq!(single.keyframe, temp.0.join(K2));
    assert_eq!(single.retry_generation, 0);
    let prompt = serde_json::from_str::<Value>(&single.prompt).unwrap();
    assert_eq!(
        prompt["images"],
        "image 1 is the whole frame at reduced size; image 2 is the full-resolution crop of region r1"
    );
    assert_eq!(prompt["regions"][0]["bbox"], json!([0.1, 0.7, 0.3, 0.8]));
    assert_eq!(prompt["nearby_dialogue"], "Meet at the colosseum.");
}

#[test]
fn the_region_box_comes_from_the_keyframe_frame_and_unusable_boxes_are_skipped() {
    let mut item = sighting("a", (1.0, 2.0), "作戦", K1, [0.0, 0.0, 192.0, 108.0]);
    item.frames[0].end_s = 1.5;
    item.frames.push(TextFrame {
        time_s: 1.5,
        end_s: 2.0,
        quad: rectangle([960.0, 540.0, 1920.0, 1080.0]),
        confidence: 0.9,
        surface_rgb: None,
    });
    item.keyframe = Some(TextKeyframe {
        time_s: 1.5,
        image: K1.into(),
    });
    let document = frame_document(Vec::new());
    assert_eq!(hint(&document, &item), Some([0.5, 0.5, 1.0, 1.0]));
    item.keyframe = Some(TextKeyframe {
        time_s: 9.0,
        image: K1.into(),
    });
    assert_eq!(hint(&document, &item), Some([0.0, 0.0, 0.1, 0.1]));
    item.frames[0].quad = rectangle([-192.0, -108.0, 192.0, 108.0]);
    assert_eq!(hint(&document, &item), Some([0.0, 0.0, 0.1, 0.1]));
    item.frames[0].quad = rectangle([-384.0, 0.0, -192.0, 108.0]);
    assert_eq!(hint(&document, &item), None);
    item.frames[0].quad = rectangle([192.0, 108.0, 192.0, 216.0]);
    assert_eq!(hint(&document, &item), None);
    item.frames.clear();
    assert_eq!(hint(&document, &item), None);
    let sign = sighting("b", (1.0, 2.0), "作戦", K1, [192.0, 108.0, 576.0, 216.0]);
    let sizeless = TextDocument {
        width: 0,
        ..frame_document(Vec::new())
    };
    assert_eq!(hint(&sizeless, &sign), None);
    let unframed = TextOccurrence {
        keyframe: None,
        ..sign
    };
    assert_eq!(hint(&document, &unframed), None);
}

#[test]
fn a_keyframe_with_more_than_sixty_four_regions_is_left_to_the_local_model() {
    let temp = Temporary::new();
    let signs = |count: usize| {
        frame_document(
            (0..count)
                .map(|n| {
                    let id = format!("s{n}");
                    sighting(&id, (1.0, 2.0), "作戦", K1, [192.0, 108.0, 576.0, 216.0])
                })
                .collect(),
        )
    };
    let corrections = TextCorrections::default();
    let requests = build_requests(&temp.0, &signs(MAX_REGIONS), &corrections, &[]);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].regions.len(), MAX_REGIONS);
    assert!(build_requests(&temp.0, &signs(MAX_REGIONS + 1), &corrections, &[]).is_empty());
}

#[test]
fn the_schema_is_closed_and_matches_the_answer_types() {
    let schema = schema();
    assert_closed(&schema);
    assert_eq!(schema["properties"]["regions"]["maxItems"], 64);
    assert_eq!(schema["properties"]["other_text"]["maxItems"], 16);
    let item = &schema["properties"]["regions"]["items"]["properties"];
    assert_eq!(item["id"]["maxLength"], 8);
    assert_eq!(item["bbox"]["minItems"], 4);
    assert_eq!(item["bbox"]["maxItems"], 4);
    let answer = KeyframeAnswer {
        other_text: vec![found("出口", None, 0.5, [0.0, 0.0, 0.5, 0.5])],
        ..two_answers()
    };
    let value = serde_json::to_value(&answer).unwrap();
    assert_eq!(keys(&value), keys(&schema["properties"]));
    assert_eq!(keys(&value["regions"][0]), keys(item));
    assert_eq!(
        keys(&value["other_text"][0]),
        keys(&schema["properties"]["other_text"]["items"]["properties"])
    );
    for (pointer, extra) in [
        ("", "summary"),
        ("/regions/0", "note"),
        ("/other_text/0", "id"),
    ] {
        let mut changed = value.clone();
        changed.pointer_mut(pointer).unwrap()[extra] = json!("more");
        assert!(serde_json::from_value::<KeyframeAnswer>(changed).is_err());
    }
    let mut short = value.clone();
    short["regions"][0]["bbox"] = json!([0.1, 0.1, 0.3]);
    assert!(serde_json::from_value::<KeyframeAnswer>(short).is_err());
    assert_eq!(
        serde_json::from_value::<KeyframeAnswer>(value).unwrap(),
        answer
    );
}

#[test]
fn an_answer_must_cover_every_region_exactly_once_with_valid_values() {
    let request = two_regions();
    let mut reversed = two_answers();
    reversed.regions.reverse();
    assert_eq!(checked(&request, reversed), Some(two_answers()));
    let broken: [fn(&mut KeyframeAnswer); 11] = [
        |answer| {
            answer.regions.pop();
        },
        |answer| answer.regions[1].id = "r1".into(),
        |answer| answer.regions[1].id = "r3".into(),
        |answer| answer.regions.push(answer.regions[0].clone()),
        |answer| answer.regions[0].bbox = [0.3, 0.1, 0.1, 0.2],
        |answer| answer.regions[0].bbox = [0.1, 0.1, 1.2, 0.2],
        |answer| answer.regions[0].confidence = 1.4,
        |answer| answer.regions[0].confidence = f64::NAN,
        |answer| answer.regions[0].english = Some("null".into()),
        |answer| answer.regions[0].english = Some("Oper\u{7}ation".into()),
        |answer| answer.regions[1].english = Some("Invented".into()),
    ];
    for (case, change) in broken.iter().enumerate() {
        let mut answer = two_answers();
        change(&mut answer);
        assert_eq!(checked(&request, answer), None, "case {case}");
    }
}

#[test]
fn cached_keyframe_answers_need_the_current_revision_and_a_fitting_request() {
    let temp = Temporary::new();
    let file = temp.0.join("claude-answer.json");
    write_cache(&file, &two_answers()).unwrap();
    assert_eq!(read_cache(&file, &two_regions()), Some(two_answers()));
    let single = Request {
        regions: vec![hinted(0, "a", [0.1, 0.1, 0.3, 0.2])],
        ..two_regions()
    };
    assert_eq!(read_cache(&file, &single), None);
    let old = super::super::Cached {
        revision: super::super::TRANSLATION_CACHE_REVISION - 1,
        answer: two_answers(),
    };
    std::fs::write(&file, serde_json::to_vec(&old).unwrap()).unwrap();
    assert_eq!(read_cache(&file, &two_regions()), None);
}

#[test]
fn a_valid_answer_sets_claude_readings_and_keeps_unreadable_regions_for_review() {
    let mut document = two_signs();
    document.occurrences[0].provenance.reason = "OCR evidence.".into();
    let mut answer = two_answers();
    answer.regions[0].english = Some("  Operation \n".into());
    let mut reasons = vec![None; 2];
    apply(
        &mut document,
        &two_regions(),
        Outcome::Answer(answer),
        "claude-cli/test",
        &mut reasons,
    );
    let [a, b] = &document.occurrences[..] else {
        panic!("two occurrences expected")
    };
    assert_eq!(a.japanese, "作戦");
    assert_eq!(a.english.as_deref(), Some("Operation"));
    assert_eq!(a.confidence, 0.95);
    assert_eq!(a.provenance.backend, "claude-cli/test");
    assert_eq!(
        a.provenance.reason,
        "OCR evidence. Translation: Read 作戦 on the frame."
    );
    assert!(a.warnings.is_empty());
    assert_eq!((b.japanese.as_str(), b.english.as_deref()), ("", None));
    assert_eq!(b.confidence, 0.9);
    assert_eq!(b.provenance.backend, "claude-cli/test");
    assert!(b.warnings.is_empty());
    assert_eq!(
        reasons,
        [
            Some("Read 作戦 on the frame.".to_string()),
            Some("Only a painted pattern.".to_string()),
        ]
    );
}

#[test]
fn a_box_far_from_the_detector_is_flagged() {
    let mut document = two_signs();
    let mut answer = two_answers();
    answer.regions[0].bbox = [0.6, 0.1, 0.8, 0.2];
    answer.regions[1].bbox = [0.52, 0.5, 0.72, 0.6];
    let mut reasons = vec![None; 2];
    apply(
        &mut document,
        &two_regions(),
        Outcome::Answer(answer),
        "claude-cli/test",
        &mut reasons,
    );
    assert_eq!(document.occurrences[0].warnings, [LOCATED_ELSEWHERE]);
    assert!(document.occurrences[1].warnings.is_empty());
}

#[test]
fn unlisted_writing_becomes_nearby_occurrences_and_invalid_entries_are_skipped() {
    let mut document = two_signs();
    let mut answer = two_answers();
    answer.other_text = vec![
        found("営業中", Some(" Open "), 0.9, [0.5, 0.25, 0.6, 0.3]),
        found(" ", None, 0.9, [0.1, 0.1, 0.2, 0.2]),
        found("出口", Some("Exit"), 0.9, [0.6, 0.1, 0.5, 0.2]),
        found("出口", Some("Exit"), 0.7, [0.7, 0.8, 0.8, 0.9]),
        // Region `a` reported again as unlisted writing: a repeat, not a new occurrence.
        found("作戦", Some("Operation"), 0.9, [0.11, 0.1, 0.3, 0.21]),
    ];
    let mut reasons = vec![None; 2];
    apply(
        &mut document,
        &two_regions(),
        Outcome::Answer(answer),
        "claude-cli/test",
        &mut reasons,
    );
    assert_eq!(document.occurrences.len(), 4);
    assert_eq!(reasons.len(), 4);
    let open = &document.occurrences[2];
    assert_eq!(open.id, "a-c1");
    assert_eq!((open.start_s, open.end_s), (3.0, 6.0));
    assert_eq!(open.japanese, "営業中");
    assert_eq!(open.english.as_deref(), Some("Open"));
    assert_eq!(open.confidence, 0.9);
    assert_eq!(open.crops, [PathBuf::from(K1)]);
    assert_eq!(open.keyframe, document.occurrences[0].keyframe);
    assert_eq!(open.presentation.treatment, TextTreatment::Nearby);
    assert_eq!(open.warnings, [FOUND_BY_CLAUDE]);
    assert_eq!(
        open.provenance,
        TextProvenance {
            backend: "claude-cli/test".into(),
            reference: None,
            reason: "Translation: Unlisted 営業中.".into(),
        }
    );
    assert!(!open.reviewed && open.rendered.is_none() && open.source_fingerprint.is_none());
    let [frame] = &open.frames[..] else {
        panic!("one frame expected")
    };
    assert_eq!(
        (
            frame.time_s,
            frame.end_s,
            frame.confidence,
            frame.surface_rgb
        ),
        (3.0, 6.0, 0.9, None)
    );
    assert!(frame.quad.valid());
    let (left, top, right, bottom) = frame.quad.bounds();
    for (value, expected) in [
        (left, 960.0),
        (top, 270.0),
        (right, 1152.0),
        (bottom, 324.0),
    ] {
        assert!((value - expected).abs() < 1e-9, "{value} != {expected}");
    }
    let exit = &document.occurrences[3];
    assert_eq!(exit.id, "a-c4");
    assert_eq!(exit.english.as_deref(), Some("Exit"));
    assert_eq!(reasons[2].as_deref(), Some("Unlisted 営業中."));
    assert_eq!(reasons[3].as_deref(), Some("Unlisted 出口."));
}

#[test]
fn a_warning_marks_every_region_and_leaves_them_for_the_local_model() {
    let mut document = two_signs();
    let original = document.clone();
    let mut reasons = vec![None::<String>; 2];
    apply(
        &mut document,
        &two_regions(),
        Outcome::Warning(INVALID_RESPONSE.into()),
        "claude-cli/test",
        &mut reasons,
    );
    for (item, before) in document.occurrences.iter().zip(&original.occurrences) {
        assert_eq!(item.warnings, [INVALID_RESPONSE]);
        assert_eq!(item.japanese, before.japanese);
        assert!(item.provenance.backend.is_empty());
    }
    assert_eq!(reasons, [None, None]);
}

#[test]
fn intersection_over_union_measures_box_agreement() {
    let unit = [0.0, 0.0, 1.0, 1.0];
    assert_eq!(intersection_over_union(unit, unit), 1.0);
    assert_eq!(
        intersection_over_union([0.0, 0.0, 0.2, 0.2], [0.5, 0.5, 0.7, 0.7]),
        0.0
    );
    let third = intersection_over_union([0.0, 0.0, 0.2, 0.1], [0.1, 0.0, 0.3, 0.1]);
    assert!((third - 1.0 / 3.0).abs() < 1e-12);
    assert_eq!(intersection_over_union([0.2; 4], [0.2; 4]), 0.0);
}
