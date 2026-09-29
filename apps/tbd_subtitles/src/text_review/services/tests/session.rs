//! Visual review artifact loading, correction durability and targeted retry invalidation.

use std::fs;
use std::path::PathBuf;

use job_model::job::JobSettings;
use job_model::onscreen::{Point, TextOccurrence, TextPresentation, TextSettings, TextTreatment};
use job_model::outputs::{AudioStream, ProbeResult, VideoStream};
use pipeline::resume::fingerprint_in_work;

use super::*;

struct Fixture {
    root: PathBuf,
    work: WorkDir,
    record: JobRecord,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("tbd-text-session-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture directory");
        let work = WorkDir::new(root.join("work"));
        let video = root.join("episode.mkv");
        fs::write(&video, b"source video stays untouched").expect("source video");
        let ass = video.with_extension("ass");
        fs::write(&ass, b"[Script Info]\nScriptType: v4.00+\n").expect("exported ASS");
        let mut settings = JobSettings::with_glossary(Vec::new());
        settings.onscreen_text = TextSettings::new_job();
        let record = JobRecord {
            video: video.to_string_lossy().into_owned(),
            video_size: 28,
            video_modified_s: 0,
            settings,
            models_dir: Some(root.join("models").to_string_lossy().into_owned()),
            corrections: None,
            steps: Default::default(),
        };
        work_dir::write_json(&work.job_json(), &record).expect("job record");
        let track = AudioStream {
            index: 2,
            audio_position: 1,
            codec: "aac".into(),
            language: Some("eng".into()),
            channels: 2,
            sample_rate: 48_000,
            start_time_s: 0.0,
        };
        work_dir::write_json(
            &work.probe(),
            &ProbeDecoded {
                probe: ProbeResult {
                    duration_s: 30.0,
                    video: Some(VideoStream {
                        index: 0,
                        codec: "h264".into(),
                        width: 1920,
                        height: 1080,
                        frame_rate_num: 24_000,
                        frame_rate_den: 1001,
                        start_time_s: 0.0,
                    }),
                    audio: vec![track.clone()],
                },
                track,
                samples: 480_000,
            },
        )
        .expect("probe record");
        work_dir::write_json(
            &work.output_record(),
            &OutputRecord {
                path: ass.to_string_lossy().into_owned(),
                ..OutputRecord::default()
            },
        )
        .expect("output record");
        work_dir::write_json(
            &work.text(StepName::TextTypeset),
            &TextDocument {
                review_warnings: Vec::new(),
                proxy_width: 0,
                sample_step: 0,
                width: 1920,
                height: 1080,
                decoded_frames: 720,
                occurrences: vec![occurrence("board"), occurrence("name")],
            },
        )
        .expect("visual results");
        Self { root, work, record }
    }

    fn load(&self) -> Session {
        load(self.work.root()).unwrap_or_else(|error| panic!("load session: {error}"))
    }

    fn error(&self) -> String {
        load(self.work.root()).expect_err("loading must fail")
    }

    fn corrections(&self) -> TextCorrections {
        corrections(&self.work).expect("saved corrections")
    }

    fn put_corrections(&self, corrections: &TextCorrections) {
        work_dir::write_json(&self.work.text_corrections(), corrections).expect("corrections");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn occurrence(id: &str) -> TextOccurrence {
    TextOccurrence {
        source_fingerprint: None,
        id: id.into(),
        start_s: 10.0,
        end_s: 12.0,
        japanese: "レベッカ".into(),
        english: Some("Rebecca".into()),
        confidence: 0.8,
        crops: Vec::new(),
        frames: Vec::new(),
        provenance: Default::default(),
        presentation: Default::default(),
        warnings: vec!["Check text".into()],
        reviewed: false,
        rendered: Some(true),
        keyframe: None,
    }
}

fn edit(english: &str) -> TextEdit {
    TextEdit {
        source_fingerprint: TextEdit::from_occurrence(&occurrence("board")).source_fingerprint,
        english: Some(english.into()),
        start_s: 10.125,
        end_s: 12.875,
        presentation: TextPresentation {
            treatment: TextTreatment::Nearby,
            anchor: Some(Point { x: 720.0, y: 400.0 }),
            font_size: Some(46.0),
        },
    }
}

#[test]
fn save_rejects_stale_and_unidentified_drafts_without_touching_corrections() {
    let fixture = Fixture::new("stale-draft");
    fixture.put_corrections(&TextCorrections {
        edits: [("name".into(), edit("Existing correction"))].into(),
        retry: vec![],
    });
    let before = fs::read(fixture.work.text_corrections()).unwrap();
    for identity in [None, Some("another-source".into())] {
        let mut session = fixture.load();
        let mut draft = edit("Wrong sign wording");
        draft.source_fingerprint = identity;
        session.draft = Some(draft);
        let error = save(&session, &Event::Save).unwrap_err();
        assert!(error.contains("does not match the current source text"));
        assert_eq!(fs::read(fixture.work.text_corrections()).unwrap(), before);
    }
}

#[test]
fn unmatched_corrections_can_be_discarded_without_a_selected_occurrence() {
    let fixture = Fixture::new("discard-orphans");
    let mut session = fixture.load();
    session.document.occurrences.clear();
    fixture.put_corrections(&TextCorrections {
        edits: [("orphan".into(), edit("Saved wording"))].into(),
        retry: vec![],
    });
    save(&session, &Event::DiscardOrphans).unwrap();
    assert!(fixture.corrections().edits.is_empty());
}

#[test]
fn old_job_without_visual_results_explains_how_to_enable_them() {
    let fixture = Fixture::new("old-job");
    let mut old = serde_json::to_value(&fixture.record).expect("old job JSON");
    old["settings"]
        .as_object_mut()
        .expect("settings")
        .remove("onscreen_text");
    work_dir::write_json(&fixture.work.job_json(), &old).expect("legacy job record");
    fs::remove_file(fixture.work.text(StepName::TextTypeset)).expect("no visual result");
    let error = fixture.error();
    assert!(error.contains("no on-screen text results"));
    assert!(error.contains("Settings") && error.contains("run the video again"));
}

#[test]
fn malformed_latest_visual_result_does_not_fall_back_to_an_older_review() {
    let fixture = Fixture::new("malformed-result");
    fs::copy(
        fixture.work.text(StepName::TextTypeset),
        fixture.work.text(StepName::TextReview),
    )
    .expect("older valid review");
    fs::write(fixture.work.text(StepName::TextTypeset), b"{broken").expect("broken JSON");
    let error = fixture.error();
    assert!(
        error.contains("cannot parse") && error.contains("text_typeset"),
        "{error}"
    );
}

#[test]
fn missing_export_record_or_file_explains_how_to_regenerate_it() {
    let fixture = Fixture::new("missing-record");
    fs::remove_file(fixture.work.output_record()).expect("remove record");
    let error = fixture.error();
    assert!(error.contains("Finish or rerun") && error.contains("ASS"));

    let fixture = Fixture::new("missing-ass");
    fs::remove_file(Path::new(&fixture.record.video).with_extension("ass")).expect("remove ASS");
    let error = fixture.error();
    assert!(error.contains("ASS file is missing") && error.contains("regenerate"));
}

#[test]
fn save_reloads_wording_timing_placement_size_and_treatment_without_touching_source_artifacts() {
    let fixture = Fixture::new("round-trip");
    let mut session = fixture.load();
    let expected = edit("Exclusive Gladiator of Corrida Colosseum\\NRebecca");
    let unchanged = [
        fixture.work.job_json(),
        fixture.work.probe(),
        fixture.work.text(StepName::TextTypeset),
        PathBuf::from(&fixture.record.video),
        session.ass.clone(),
    ]
    .map(|path| {
        let bytes = fs::read(&path).expect("original artifact");
        (path, bytes)
    });
    session.draft = Some(expected.clone());
    save(&session, &Event::Save).expect("save correction");
    let reloaded = fixture.load();
    assert_eq!(reloaded.draft.as_ref(), Some(&expected));
    assert_eq!(reloaded.corrections.edits.get("board"), Some(&expected));
    assert_eq!(reloaded.position_s, expected.start_s);
    assert_eq!(reloaded.audio_position, 1);
    assert_eq!(reloaded.fps, 24_000.0 / 1001.0);
    for (path, bytes) in unchanged {
        assert_eq!(fs::read(path).expect("unchanged artifact"), bytes);
    }
}

#[test]
fn undo_rereads_current_corrections_and_preserves_other_occurrences() {
    let fixture = Fixture::new("undo");
    let session = fixture.load();
    let other = edit("SOP Operation");
    fixture.put_corrections(&TextCorrections {
        edits: [
            ("board".into(), edit("Dressrosa")),
            ("name".into(), other.clone()),
        ]
        .into(),
        retry: vec!["name".into()],
    });
    save(&session, &Event::Undo).expect("undo selected occurrence");
    let current = fixture.corrections();
    assert!(!current.edits.contains_key("board"));
    assert_eq!(current.edits.get("name"), Some(&other));
    assert_eq!(current.retry, ["name"]);
    assert_eq!(
        fixture.load().draft,
        Some(TextEdit::from_occurrence(&occurrence("board")))
    );
}

#[test]
fn each_retry_changes_read_and_translation_fingerprints_without_discarding_edits() {
    let fixture = Fixture::new("repeated-retry");
    let session = fixture.load();
    let saved = TextCorrections {
        edits: [
            ("board".into(), edit("Dressrosa")),
            ("name".into(), edit("Rebecca")),
        ]
        .into(),
        retry: vec!["name".into()],
    };
    fixture.put_corrections(&saved);
    let fingerprints = || {
        [StepName::TextRead, StepName::TextTranslate]
            .map(|step| fingerprint_in_work(step, &fixture.record, &fixture.work))
    };
    let audio = fingerprint_in_work(StepName::Cues, &fixture.record, &fixture.work);
    let before = fingerprints();
    save(&session, &Event::Retry).expect("first retry");
    let first = fingerprints();
    save(&session, &Event::Retry).expect("second retry");
    let second = fingerprints();
    for index in 0..2 {
        assert_ne!(before[index], first[index]);
        assert_ne!(first[index], second[index]);
    }
    let current = fixture.corrections();
    assert_eq!(current.retry, ["name", "board", "board"]);
    assert_eq!(current.edits, saved.edits);
    assert_eq!(
        fingerprint_in_work(StepName::Cues, &fixture.record, &fixture.work),
        audio
    );
}

#[test]
fn invalid_timing_size_and_placement_fail_before_creating_or_replacing_corrections() {
    let fixture = Fixture::new("invalid-bounds");
    let mut session = fixture.load();
    let valid = edit("Dressrosa");
    let mut invalid = Vec::new();
    for (start_s, end_s) in [(-1.0, 12.0), (12.0, 10.0), (10.0, 31.0), (f64::NAN, 12.0)] {
        invalid.push(TextEdit {
            start_s,
            end_s,
            ..valid.clone()
        });
    }
    for size in [7.0, 401.0, f64::INFINITY] {
        let mut bad = valid.clone();
        bad.presentation.font_size = Some(size);
        invalid.push(bad);
    }
    for (x, y) in [
        (-1.0, 10.0),
        (1921.0, 10.0),
        (10.0, 1081.0),
        (f64::NAN, 10.0),
    ] {
        let mut bad = valid.clone();
        bad.presentation.anchor = Some(Point { x, y });
        invalid.push(bad);
    }
    for bad in &invalid {
        session.draft = Some(bad.clone());
        assert!(save(&session, &Event::Save).is_err());
    }
    let path = fixture.work.text_corrections();
    assert!(!path.exists());
    assert!(!path.with_extension("json.lock").exists());
    fixture.put_corrections(&TextCorrections {
        edits: [("name".into(), valid)].into(),
        retry: vec![],
    });
    let before = fs::read(&path).expect("existing corrections");
    for bad in invalid {
        session.draft = Some(bad);
        assert!(save(&session, &Event::Save).is_err());
        assert_eq!(fs::read(&path).expect("preserved corrections"), before);
    }
}

#[test]
fn malformed_corrections_are_reported_and_never_overwritten() {
    let fixture = Fixture::new("malformed-corrections");
    let session = fixture.load();
    let path = fixture.work.text_corrections();
    fs::write(&path, b"{invalid correction").expect("broken corrections");
    assert!(fixture.error().contains("Cannot read visual corrections"));
    assert!(save(&session, &Event::Retry).is_err());
    assert_eq!(
        fs::read(path).expect("preserved malformed file"),
        b"{invalid correction"
    );
}
