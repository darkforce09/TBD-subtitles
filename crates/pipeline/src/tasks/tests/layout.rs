use job_model::outputs::{Chosen, Correction, Findings};

use super::*;

fn utterance(id: &str, heard: &[&str]) -> Utterance {
    Utterance {
        id: id.into(),
        start_s: 0.0,
        end_s: 1.0,
        words: Vec::new(),
        locked: Vec::new(),
        line: String::new(),
        hypotheses: vec![("P".into(), heard.iter().map(|s| s.to_string()).collect())],
    }
}

fn line(id: &str, t: &str) -> Line {
    Line {
        id: id.into(),
        t: t.into(),
        f: Vec::new(),
    }
}

fn correction(id: &str, text: &str, chosen: Chosen) -> Correction {
    Correction {
        id: id.into(),
        text: text.into(),
        flags: Vec::new(),
        chosen,
    }
}

#[test]
fn owner_lines_are_settled_and_fix_it_lines_are_checked_again() {
    let sheet = vec![
        utterance("U1", &["Go", "Luffy"]),
        utterance("U2", &["Hi"]),
        utterance("U8", &["Far", "away"]),
        utterance("U9", &["Stop", "it"]),
    ];
    let pass = AdjudicationPass {
        lines: vec![
            line("U1", "Go, Lufy!"),
            line("U2", "Hi."),
            line("U8", "Far away."),
            line("U9", "Stop it."),
        ],
        findings: Findings {
            novel: vec![("U1".into(), "lufy".into()), ("U2".into(), "hey".into())],
            removed_locked: vec![("U2".into(), "Hi".into()), ("U9".into(), "it".into())],
            ..Findings::default()
        },
        ..AdjudicationPass::default()
    };
    let fix = || Chosen::FixIt {
        model: "opus".into(),
        why: "x".into(),
    };
    let corrections = Corrections {
        lines: vec![
            correction("U1", "Go, Luffy!", fix()),
            correction("U2", "Hey there.", Chosen::Typed),
            correction("U9", "Stop it now.", fix()),
        ],
    };
    let out = settled(pass, &corrections, &sheet, &[]);
    assert_eq!(out.lines[0].t, "Go, Luffy!");
    // U1's fix is heard; U9's fix has a word no engine heard near it.
    assert_eq!(out.findings.novel, [("U9".to_string(), "now".to_string())]);
    assert!(out.findings.removed_locked.is_empty());
}

/// A quad from `(left, top)` to `(right, bottom)` in video pixels, as the contract serializes it.
fn quad(left: f64, top: f64, right: f64, bottom: f64) -> serde_json::Value {
    serde_json::json!([
        {"x": left, "y": top},
        {"x": right, "y": top},
        {"x": right, "y": bottom},
        {"x": left, "y": bottom},
    ])
}

/// An occurrence from `start_s` to `end_s`, with one sampled frame per `(time_s, quad)`.
fn occurrence(
    id: &str,
    start_s: f64,
    end_s: f64,
    frames: &[(f64, serde_json::Value)],
) -> serde_json::Value {
    let frames: Vec<serde_json::Value> = frames
        .iter()
        .map(|(time_s, quad)| {
            serde_json::json!({
                "time_s": time_s, "end_s": time_s + 0.5, "quad": quad,
                "confidence": 0.9, "surface_rgb": null,
            })
        })
        .collect();
    serde_json::json!({
        "id": id, "start_s": start_s, "end_s": end_s, "japanese": "看板",
        "english": "Sign", "confidence": 0.9, "crops": [], "frames": frames,
        "provenance": {"backend": "claude", "reference": null, "reason": ""},
        "presentation": {"treatment": "auto", "anchor": null, "font_size": null},
        "warnings": [],
    })
}

fn text_document(occurrences: Vec<serde_json::Value>) -> TextDocument {
    let mut value = serde_json::to_value(TextDocument::default()).unwrap();
    value["occurrences"] = serde_json::Value::Array(occurrences);
    serde_json::from_value(value).unwrap()
}

/// A 1280 × 720 composition: `baked` drawn into the video, `fallback` left Japanese.
fn composition(baked: &str, fallback: &str) -> ReplacementDocument {
    serde_json::from_value(serde_json::json!({
        "width": 1280, "height": 720, "frame_count": 480,
        "texts": [
            {"id": baked, "first_frame": 12, "last_frame": 96, "status": {"kind": "baked"}},
            {
                "id": fallback, "first_frame": 216, "last_frame": 312,
                "status": {"kind": "fallback", "reason": "the card is too busy"},
            },
        ],
    }))
    .unwrap()
}

#[test]
fn lettered_writing_follows_each_sampled_frame_on_the_ass_canvas() {
    let text = text_document(vec![
        occurrence(
            "moving",
            1.0,
            4.0,
            &[
                (1.2, quad(100.0, 600.0, 300.0, 700.0)),
                (2.5, quad(200.0, 600.0, 400.0, 700.0)),
            ],
        ),
        occurrence("left", 9.0, 13.0, &[(9.0, quad(0.0, 0.0, 10.0, 10.0))]),
    ]);
    let obstacles = lettered_writing(&composition("moving", "left"), &text);
    assert_eq!(
        obstacles,
        vec![
            Obstacle {
                start_s: 1.0,
                end_s: 2.5,
                rect: [138.0, 888.0, 462.0, 1062.0],
            },
            Obstacle {
                start_s: 2.5,
                end_s: 4.0,
                rect: [288.0, 888.0, 612.0, 1062.0],
            },
        ],
        "scaled by 1.5, grown by 12 pixels; writing left in Japanese is no obstacle"
    );
}

#[test]
fn a_refitted_lettering_area_is_the_obstacle_for_its_whole_span() {
    let text = text_document(vec![occurrence(
        "claude",
        1.0,
        4.0,
        &[(1.0, quad(0.0, 0.0, 600.0, 200.0))],
    )]);
    let mut replacement = composition("claude", "left");
    replacement.texts[0].lettering_quad =
        Some(serde_json::from_value(quad(100.0, 600.0, 300.0, 700.0)).unwrap());
    assert_eq!(
        lettered_writing(&replacement, &text),
        vec![Obstacle {
            start_s: 1.0,
            end_s: 4.0,
            rect: [138.0, 888.0, 462.0, 1062.0],
        }],
        "the English sits where the ink was found, not in the box the reader gave"
    );
}

#[test]
fn the_localized_subtitles_carry_dialogue_alone_moved_above_lettered_writing() {
    use job_model::job::{JobRecord, JobSettings};
    use subtitle_formats::cue::{Cue, CueKind, CueLine};
    let scratch = crate::work_dir::store::scratch::Scratch::new("layout-localized");
    let (store, dir) = (scratch.store(), scratch.dir.clone());
    let mut settings = JobSettings::with_glossary(vec![]);
    settings.onscreen_text.enabled = true;
    settings.onscreen_text.localized_video = true;
    let video = dir.join("episode.mkv");
    let job = Job {
        library: None,
        work: scratch.work().clone(),
        record: JobRecord {
            video: video.to_string_lossy().into_owned(),
            video_size: 1,
            video_modified_s: 0,
            settings,
            models_dir: Some("/no/models".into()),
            corrections: None,
        },
    };
    let cue = |start, end, text: &str| Cue {
        start,
        end,
        lines: vec![CueLine::plain(text)],
        kind: CueKind::Dialogue,
    };
    let track = CueTrack {
        frame_rate: FrameRate::FILM,
        cues: vec![cue(24, 72, "Who's there?"), cue(240, 288, "Nobody.")],
    };
    store.put_output(StepName::Cues, None, &track).unwrap();
    let sign = "Dialogue: 0,0:00:01.00,0:00:03.00,Sign,,0,0,0,,Rebecca\n";
    store
        .put_output(
            StepName::TextTypeset,
            Some(keys::TYPESET_ASS),
            &sign.to_string(),
        )
        .unwrap();
    let lower_third = quad(400.0, 620.0, 880.0, 700.0);
    let text = text_document(vec![
        occurrence("card", 0.5, 4.0, &[(0.5, lower_third.clone())]),
        occurrence("busy", 9.0, 13.0, &[(9.0, lower_third)]),
    ]);
    store
        .put_output(StepName::TextTypeset, None, &text)
        .unwrap();
    let verified = job_model::onscreen::VerifiedReplacements {
        document: composition("card", "busy"),
        checks: Vec::new(),
    };
    store
        .put_output(StepName::TextVerify, None, &verified)
        .unwrap();

    let mut io = crate::tasks::StepIo::in_process(store).unwrap();
    let report = output(&job, &mut io, &|_, _| {}).expect("output");
    let normal = std::fs::read_to_string(dir.join("episode.ass")).unwrap();
    let localized = std::fs::read_to_string(dir.join("episode.localized.ass")).unwrap();
    assert!(normal.ends_with(sign), "{normal}");
    assert!(!normal.contains("\\an8"), "{normal}");
    assert!(
        !localized.contains("Rebecca"),
        "no on-screen events: {localized}"
    );
    assert!(
        localized.ends_with(
            "Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\an8}Who's there?\n\
             Dialogue: 0,0:00:10.00,0:00:12.00,Default,,0,0,0,,Nobody.\n"
        ),
        "{localized}"
    );
    assert_eq!(
        report.notes.get("localized_moved_up").map(String::as_str),
        Some("1")
    );

    drop(io);
    let mut write = store.write().unwrap();
    let verify = keys::output_key(StepName::TextVerify, None);
    assert!(
        write
            .remove(worker_channel::address::Table::Outputs, &verify)
            .unwrap()
    );
    write.commit().unwrap();
    let mut io = crate::tasks::StepIo::in_process(store).unwrap();
    assert!(
        output(&job, &mut io, &|_, _| {}).is_err(),
        "the localized subtitles need the checked replacements"
    );
}
