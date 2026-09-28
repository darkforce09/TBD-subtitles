use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use inference::llm::{Completion, LlmError};
use job_model::StepName;
use job_model::job::{JobRecord, JobSettings, StepMeasure, StepRecord};
use job_model::outputs::{
    AdjudicationPass, Aligned, Chosen, ChunkWords, Correction, EngineTranscript, Line, TimeSpan,
    TimedWord, Utterance,
};
use job_model::report::{QcCheck, QcFinding, QcReport};
use serde_json::{Value, json};
use stages::fix_it::prompt;

use super::*;

type Script = dyn Fn(&str, &str) -> std::result::Result<Value, String> + Send + Sync;

struct Scripted {
    script: Arc<Script>,
    calls: Arc<AtomicUsize>,
}

impl LanguageModel for Scripted {
    fn name(&self) -> String {
        "scripted".into()
    }

    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        _: &Value,
    ) -> std::result::Result<Completion, LlmError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        (self.script)(system, user)
            .map(|json| Completion {
                json,
                input_tokens: 10,
                output_tokens: 1,
                cost_usd: Some(0.5),
            })
            .map_err(LlmError)
    }
}

fn maker(
    script: Arc<Script>,
    calls: Arc<AtomicUsize>,
) -> impl Fn() -> Box<dyn LanguageModel + Send> + Sync {
    move || {
        Box::new(Scripted {
            script: script.clone(),
            calls: calls.clone(),
        }) as Box<dyn LanguageModel + Send>
    }
}

/// Brief, then "Uh" put back in U2 and U1 kept as it is, then every change accepted.
fn answer(system: &str, user: &str) -> std::result::Result<Value, String> {
    if system.starts_with(prompt::BRIEF) {
        return Ok(
            json!({"show": "One Piece", "episode": "Dressrosa 12", "cast": [],
            "summary": "A question.", "speech_habits": [], "suspects": []}),
        );
    }
    if system == prompt::JUDGE {
        return Ok(json!({"verdicts": [{"id": "U2", "accept": true, "why": "heard"}]}));
    }
    assert!(user.contains("### U2"), "{user}");
    Ok(json!({"lines": [
        {"id": "U1", "t": "Look!", "f": [], "why": "right as it is"},
        {"id": "U2", "t": "Uh, go!", "f": ["SPK"], "why": "P heard Uh"}
    ]}))
}

fn utterance(id: &str, start_s: f64, end_s: f64, p: &[&str], w: &[&str]) -> Utterance {
    let words = |ws: &[&str]| ws.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    Utterance {
        id: id.into(),
        start_s,
        end_s,
        words: Vec::new(),
        locked: Vec::new(),
        line: String::new(),
        hypotheses: vec![("P".into(), words(p)), ("W".into(), words(w))],
    }
}

/// A finished job of three lines whose second lost a heard "Uh", in a scratch work root.
fn finished_job(name: &str) -> (WorkDir, PathBuf, FixOptions) {
    let root = std::env::temp_dir().join(format!("tbd-fix-it-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let video = root.join("one_pace").join("[Muhn Pace] Dressrosa 12.mp4");
    let work = WorkDir::new(root.join("work").join(work_dir::job_id(&video)));
    let step = StepRecord {
        fingerprint: String::new(),
        finished_ns: 0,
        measure: StepMeasure::default(),
    };
    let record = JobRecord {
        video: video.to_string_lossy().into_owned(),
        video_size: 0,
        video_modified_s: 0,
        settings: JobSettings::with_glossary(vec!["Rebecca".into()]),
        models_dir: None,
        corrections: None,
        steps: BTreeMap::from([(StepName::Qc, step.clone()), (StepName::Output, step)]),
    };
    work_dir::write_json(&work.job_json(), &record).unwrap();
    let sheet = vec![
        utterance("U1", 0.0, 1.0, &["Look!"], &["Look!"]),
        utterance("U2", 1.5, 3.0, &["Uh", "go!"], &["Go!"]),
        utterance("U3", 5.0, 6.0, &["Now!"], &["Now!"]),
    ];
    work_dir::write_json(&work.sheet(), &sheet).unwrap();
    let line = |id: &str, t: &str, f: &[&str]| Line {
        id: id.into(),
        t: t.into(),
        f: f.iter().map(|s| s.to_string()).collect(),
    };
    let adjudicated = AdjudicationPass {
        lines: vec![
            line("U1", "Look!", &[]),
            line("U2", "Go!", &["SPK"]),
            line("U3", "Now!", &[]),
        ],
        ..AdjudicationPass::default()
    };
    work_dir::write_json(&work.adjudicated(), &adjudicated).unwrap();
    let qc = QcReport {
        findings: vec![QcFinding {
            check: QcCheck::UncoveredSpeech,
            time_s: 1.5,
            text: String::new(),
            detail: "0.4 s".into(),
            utterance: None,
        }],
        ..QcReport::default()
    };
    work_dir::write_json(&work.qc(), &qc).unwrap();
    work_dir::write_json(&work.reviewed(), &Aligned::default()).unwrap();
    let heard = EngineTranscript {
        chunks: vec![ChunkWords {
            span: TimeSpan::new(0.0, 6.0),
            words: vec![TimedWord {
                text: "Uh".into(),
                start_s: 1.5,
                end_s: 1.8,
                confidence: None,
            }],
        }],
        ..EngineTranscript::default()
    };
    work_dir::write_json(&work.asr("parakeet"), &heard).unwrap();
    let options = FixOptions {
        work_root: root.join("work"),
        model: "opus".into(),
        glossary_name: "one_piece".into(),
        processes: 2,
        cancel: CancelToken::new(),
    };
    (work, video, options)
}

#[test]
fn kept_changes_become_fix_it_corrections_and_the_run_is_recorded() {
    let (work, video, options) = finished_job("kept");
    let calls = Arc::new(AtomicUsize::new(0));
    let make = maker(Arc::new(answer), calls.clone());
    let stages = Mutex::new(Vec::new());
    let outcome = fix_job(&video, &work, &options, &make, &|p| {
        stages.lock().unwrap().push(p.stage)
    })
    .unwrap();
    assert_eq!(outcome.changed, ["U1", "U2"]);
    let corrections = work_dir::read_corrections(&work).unwrap();
    let u2 = corrections.get("U2").unwrap();
    assert_eq!(u2.text, "Uh, go!");
    assert!(
        matches!(&u2.chosen, Chosen::FixIt { model, why } if model == "opus" && why == "P heard Uh")
    );
    let u1 = corrections.get("U1").unwrap();
    assert_eq!(u1.text, "Look!");
    let record: FixRecord = work_dir::read_json(&work.fix_record()).unwrap();
    assert!(record.lines.iter().all(|l| l.applied));
    assert_eq!(record.brief.show, "One Piece");
    assert_eq!((record.calls, record.cached_calls), (3, 0));
    assert!((record.cost_usd - 1.5).abs() < 1e-9);
    assert!(!work.fix_calls().exists());
    let stages = stages.into_inner().unwrap();
    assert_eq!(stages.first(), Some(&FixStage::Reading));
    assert_eq!(stages.last(), Some(&FixStage::Saving));
    let _ = std::fs::remove_dir_all(options.work_root.parent().unwrap());
}

#[test]
fn a_correction_the_owner_saves_during_the_run_wins() {
    let (work, video, options) = finished_job("owner");
    let owner_work = work.clone();
    let script = move |system: &str, user: &str| {
        if system == prompt::JUDGE {
            work_dir::update_corrections(&owner_work, |c| {
                c.set(Correction {
                    id: "U2".into(),
                    text: "Go, go!".into(),
                    flags: Vec::new(),
                    chosen: Chosen::Typed,
                })
            })
            .unwrap();
        }
        answer(system, user)
    };
    let make = maker(Arc::new(script), Arc::new(AtomicUsize::new(0)));
    let outcome = fix_job(&video, &work, &options, &make, &|_| {}).unwrap();
    assert_eq!(outcome.kept_yours, ["U2"]);
    assert_eq!(outcome.changed, ["U1"]);
    let corrections = work_dir::read_corrections(&work).unwrap();
    assert_eq!(corrections.get("U2").unwrap().chosen, Chosen::Typed);
    let _ = std::fs::remove_dir_all(options.work_root.parent().unwrap());
}

#[test]
fn a_stopped_run_changes_nothing_and_the_next_run_reuses_its_answers() {
    let (work, video, options) = finished_job("stopped");
    let token = options.cancel.clone();
    let script = move |system: &str, user: &str| {
        if !system.starts_with(prompt::BRIEF) && system != prompt::JUDGE {
            token.cancel();
        }
        answer(system, user)
    };
    let make = maker(Arc::new(script), Arc::new(AtomicUsize::new(0)));
    let stopped = fix_job(&video, &work, &options, &make, &|_| {}).unwrap_err();
    assert!(stopped.is_cancelled());
    assert!(!work.review().exists());
    assert!(work.fix_calls().read_dir().unwrap().count() >= 2);

    let again = FixOptions {
        cancel: CancelToken::new(),
        ..options.clone()
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let make = maker(Arc::new(answer), calls.clone());
    let outcome = fix_job(&video, &work, &again, &make, &|_| {}).unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "only the judge is asked again"
    );
    assert_eq!(outcome.record.cached_calls, 2);
    let _ = std::fs::remove_dir_all(options.work_root.parent().unwrap());
}

#[test]
fn a_job_that_is_not_ready_is_refused() {
    let (work, video, options) = finished_job("refused");
    let make = maker(Arc::new(answer), Arc::new(AtomicUsize::new(0)));
    work_dir::update_corrections(&work, |c| {
        c.set(Correction {
            id: "U3".into(),
            text: "Now.".into(),
            flags: Vec::new(),
            chosen: Chosen::Typed,
        })
    })
    .unwrap();
    let error = fix_job(&video, &work, &options, &make, &|_| {}).unwrap_err();
    assert!(
        error.message.contains("not in the subtitles yet"),
        "{error}"
    );
    let mut record: JobRecord = work_dir::read_json(&work.job_json()).unwrap();
    record.corrections = work_dir::corrections_digest(&work);
    record.steps.remove(&StepName::Output);
    work_dir::write_json(&work.job_json(), &record).unwrap();
    let error = fix_job(&video, &work, &options, &make, &|_| {}).unwrap_err();
    assert!(error.message.contains("no finished subtitles"), "{error}");
    let _ = std::fs::remove_dir_all(options.work_root.parent().unwrap());
}

fn finding(check: QcCheck, id: Option<&str>, time_s: f64, detail: &str) -> QcFinding {
    QcFinding {
        check,
        time_s,
        text: String::new(),
        detail: detail.into(),
        utterance: id.map(str::to_string),
    }
}

/// The job as a correction run leaves it: its subtitles hold every correction, its quality check
/// finds `findings`, and its re-adjudication has `fingerprint`.
fn checked_again(work: &WorkDir, findings: Vec<QcFinding>, fingerprint: &str) -> QcReport {
    let mut record: JobRecord = work_dir::read_json(&work.job_json()).unwrap();
    record.corrections = work_dir::corrections_digest(work);
    let step = StepRecord {
        fingerprint: fingerprint.into(),
        finished_ns: 0,
        measure: StepMeasure::default(),
    };
    record.steps.insert(StepName::Readjudicate, step);
    work_dir::write_json(&work.job_json(), &record).unwrap();
    let qc = QcReport {
        findings,
        ..QcReport::default()
    };
    work_dir::write_json(&work.qc(), &qc).unwrap();
    qc
}

/// Brief, then every asked line returned as it is, each repair call's ids kept in `asked`.
fn as_it_is(asked: Arc<Mutex<Vec<String>>>) -> Arc<Script> {
    Arc::new(move |system: &str, user: &str| {
        if system.starts_with(prompt::BRIEF) || system == prompt::JUDGE {
            return answer(system, user);
        }
        let text = |id: &str| match id {
            "U1" => "Look!",
            "U2" => "Uh, go!",
            _ => "Now!",
        };
        let lines: Vec<Value> = user
            .lines()
            .filter_map(|l| l.strip_prefix("### "))
            .filter_map(|l| l.split_whitespace().next())
            .map(|id| {
                asked.lock().unwrap().push(id.to_string());
                json!({"id": id, "t": text(id), "f": [], "why": "right as it is"})
            })
            .collect();
        Ok(json!({ "lines": lines }))
    })
}

#[test]
fn a_second_run_asks_only_what_is_left_and_keeps_what_the_first_answered() {
    let (work, video, options) = finished_job("second");
    let uncovered = finding(QcCheck::UncoveredSpeech, None, 1.5, "0.4 s");
    let short = |id: &str| finding(QcCheck::TooShort, Some(id), 0.0, "12 frames");
    let first_qc = checked_again(&work, vec![short("U1"), uncovered], "a1");
    let make = maker(Arc::new(answer), Arc::new(AtomicUsize::new(0)));
    let first = fix_job(&video, &work, &options, &make, &|_| {})
        .unwrap()
        .record;
    assert_eq!(first.adjudication, "a1");
    assert_eq!(first.before, Some(FixBefore::of(&first_qc)));
    let u1 = first.lines.iter().find(|l| l.id == "U1").unwrap();
    assert_eq!(u1.checks, [QcCheck::TooShort, QcCheck::UncoveredSpeech]);

    checked_again(&work, vec![short("U1"), short("U3")], "a1");
    let asked = Arc::new(Mutex::new(Vec::new()));
    let make = maker(as_it_is(asked.clone()), Arc::new(AtomicUsize::new(0)));
    let outcome = fix_job(&video, &work, &options, &make, &|_| {}).unwrap();
    assert_eq!(
        *asked.lock().unwrap(),
        ["U3"],
        "U1 was answered about being too short"
    );
    assert_eq!(outcome.changed, ["U3"]);
    let record: FixRecord = work_dir::read_json(&work.fix_record()).unwrap();
    assert_eq!(record, outcome.record);
    let ids: Vec<&str> = record.lines.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, ["U1", "U2", "U3"]);
    assert_eq!(record.lines[..2], first.lines[..]);
    assert_eq!(record.lines[2].checks, [QcCheck::TooShort]);
    assert_eq!(record.calls, 5);
    assert!((record.cost_usd - 2.5).abs() < 1e-9, "{}", record.cost_usd);
    assert_eq!(record.input_tokens, 50);
    assert_eq!(
        record.before, first.before,
        "the problems before the first run"
    );
    let _ = std::fs::remove_dir_all(options.work_root.parent().unwrap());
}

#[test]
fn a_record_from_before_the_video_was_adjudicated_again_is_ignored() {
    let (work, video, options) = finished_job("stale");
    let short = |id: &str| finding(QcCheck::TooShort, Some(id), 0.0, "12 frames");
    let uncovered = finding(QcCheck::UncoveredSpeech, None, 1.5, "0.4 s");
    checked_again(&work, vec![short("U1"), uncovered], "a1");
    let make = maker(Arc::new(answer), Arc::new(AtomicUsize::new(0)));
    fix_job(&video, &work, &options, &make, &|_| {}).unwrap();

    let second_qc = checked_again(&work, vec![short("U1"), short("U3")], "b2");
    let asked = Arc::new(Mutex::new(Vec::new()));
    let make = maker(as_it_is(asked.clone()), Arc::new(AtomicUsize::new(0)));
    let record = fix_job(&video, &work, &options, &make, &|_| {})
        .unwrap()
        .record;
    assert_eq!(*asked.lock().unwrap(), ["U1", "U3"]);
    let ids: Vec<&str> = record.lines.iter().map(|l| l.id.as_str()).collect();
    assert_eq!(ids, ["U1", "U3"]);
    assert_eq!(record.calls, 2);
    assert!((record.cost_usd - 1.0).abs() < 1e-9, "{}", record.cost_usd);
    assert_eq!(record.adjudication, "b2");
    assert_eq!(record.before, Some(FixBefore::of(&second_qc)));
    let _ = std::fs::remove_dir_all(options.work_root.parent().unwrap());
}

#[test]
fn a_run_that_cannot_make_its_brief_changes_nothing() {
    let (work, video, options) = finished_job("brief");
    let make = maker(
        Arc::new(|_: &str, _: &str| Err("not logged in".to_string())),
        Arc::new(AtomicUsize::new(0)),
    );
    let error = fix_job(&video, &work, &options, &make, &|_| {}).unwrap_err();
    assert!(error.message.contains("not logged in"), "{error}");
    assert!(!error.is_cancelled());
    assert!(!work.review().exists());
    assert!(!work.fix_record().exists());
    let _ = std::fs::remove_dir_all(options.work_root.parent().unwrap());
}
