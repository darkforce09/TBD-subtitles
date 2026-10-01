use job_model::outputs::{
    AdjudicationPass, ChunkWords, EngineTranscript, Findings, Line, Redecode, TimeSpan, TimedWord,
};
use job_model::report::{QcCheck, QcFinding};

use super::*;

pub(super) fn word(text: &str) -> TimedWord {
    TimedWord {
        text: text.into(),
        start_s: 0.0,
        end_s: 0.1,
        confidence: None,
    }
}

pub(super) fn utterance(id: &str, start_s: f64, p: &str, w: &str) -> Utterance {
    Utterance {
        id: id.into(),
        start_s,
        end_s: start_s + 1.0,
        words: vec![word(p)],
        locked: vec![false],
        line: format!("{id} | {{P:{p}|W:{w}}}"),
        hypotheses: vec![
            ("P".into(), vec![p.to_string()]),
            ("W".into(), vec![w.to_string()]),
        ],
    }
}

/// This process's handle of the database of the job in `job`.
fn store(job: &Path) -> std::sync::Arc<pipeline::work_dir::JobStore> {
    pipeline::work_dir::JobStore::open(&pipeline::work_dir::WorkDir::new(job)).expect("the store")
}

/// A work directory with two utterances; the first unsure and heard again, the second settled.
pub(super) fn job(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-review-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = store(&dir);
    let sheet = vec![
        utterance("U1", 10.0, "blame!", "flavor!"),
        utterance("U2", 12.0, "Go!", "Go!"),
    ];
    store
        .put_output(StepName::DiffSheet, None, &sheet)
        .expect("sheet");
    let adjudicated = AdjudicationPass {
        lines: vec![
            Line {
                id: "U1".into(),
                t: "Blaver!".into(),
                f: vec!["UNSURE".into()],
            },
            Line {
                id: "U2".into(),
                t: "Go!".into(),
                f: vec!["SPK".into()],
            },
        ],
        findings: Findings::default(),
        redecoded: vec!["U1".into()],
        calls: 1,
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: 0.0,
        failed_calls: vec![],
    };
    store
        .put_output(StepName::Readjudicate, None, &adjudicated)
        .expect("adjudicated");
    let redecode = Redecode {
        ids: vec!["U1".into()],
        transcript: EngineTranscript {
            engine: "parakeet".into(),
            input: "vocals".into(),
            chunks: vec![ChunkWords {
                span: TimeSpan::new(9.5, 11.5),
                words: vec![word("Brave!")],
            }],
        },
    };
    store
        .put_output(StepName::RedecodeParakeet, None, &redecode)
        .expect("redecode");
    let qc = QcReport {
        findings: vec![QcFinding {
            check: QcCheck::Unsure,
            time_s: 10.0,
            text: "Blaver!".into(),
            detail: "U1".into(),
            utterance: Some("U1".into()),
        }],
        ..QcReport::default()
    };
    store.put_output(StepName::Qc, None, &qc).expect("qc");
    dir
}

#[test]
fn every_reading_of_a_line_is_gathered_in_sheet_order() {
    let dir = job("load");
    let session = load(Path::new("/v/a.mp4"), &dir).expect("session");
    assert_eq!(session.lines.len(), 2);
    let first = &session.lines[0];
    assert_eq!(first.adjudicated, "Blaver!");
    let tags: Vec<(&str, &str)> = first
        .hypotheses
        .iter()
        .map(|h| (h.tag.as_str(), h.text.as_str()))
        .collect();
    assert_eq!(
        tags,
        [("P", "blame!"), ("W", "flavor!"), ("ALT p", "Brave!")]
    );
    assert!(first.flagged());
    assert_eq!(
        first.groups,
        [(
            LineGroup::Unsure,
            "The engines disagreed and a second listen didn't settle it. The app's best guess \
             is in the file."
                .to_string()
        )]
    );
    assert!(!session.lines[1].flagged(), "no finding names U2");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_job_without_a_sheet_names_it() {
    let error = load(Path::new("/v/a.mp4"), Path::new("/no/such/job")).expect_err("missing");
    assert!(error.contains("diff_sheet"), "{error}");
}

#[test]
fn why_a_line_is_flagged_names_the_word_or_the_number() {
    let finding = |check: QcCheck, detail: &str| QcFinding {
        check,
        time_s: 0.0,
        text: String::new(),
        detail: detail.into(),
        utterance: Some("U1".into()),
    };
    assert_eq!(
        why(&finding(QcCheck::RemovedLocked, "U1: Frankie,")),
        "Both engines heard “Frankie”; the subtitles don't use it."
    );
    assert_eq!(
        why(&finding(QcCheck::Novel, "U1: Franky")),
        "Neither engine heard “Franky”."
    );
    assert_eq!(
        why(&finding(QcCheck::TooFast, "23.4 cps")),
        "23.4 characters per second; the limit is 20."
    );
    assert_eq!(
        why(&finding(QcCheck::LineTooLong, "47 characters")),
        "Layout: line over 42 characters."
    );
}

#[test]
fn a_line_fix_it_changed_is_in_claude_s_group_first_with_what_the_app_had() {
    let dir = job("fix-it");
    put_corrections(
        &dir,
        &Corrections {
            lines: vec![job_model::outputs::Correction {
                id: "U1".into(),
                text: "Brave!".into(),
                flags: Vec::new(),
                chosen: Chosen::FixIt {
                    model: "opus".into(),
                    why: "Heard again as Brave.".into(),
                },
            }],
        },
    );
    let session = load(Path::new("/v/a.mp4"), &dir).expect("load");
    let line = session.line("U1").expect("U1");
    assert_eq!(
        line.groups[0],
        (
            LineGroup::ChangedByFixIt,
            "The app had “Blaver!”. Heard again as Brave.".to_string()
        )
    );
    assert!(session.unchecked_fix("U1"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Store `corrections` as the line corrections of the job in `job`.
fn put_corrections(job: &Path, corrections: &Corrections) {
    pipeline::work_dir::update_corrections(&store(job), |c| *c = corrections.clone())
        .expect("the corrections");
}
