use std::collections::BTreeMap;

use job_model::job::{JobSettings, StepMeasure, StepRecord};
use job_model::outputs::{Chosen, Correction, FixVerdict, LineFix};
use job_model::report::{QcCheck, QcFinding};

use super::*;

fn scratch(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("tbd-report-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("dir");
    root
}

fn write(path: &Path, value: &impl serde::Serialize) {
    std::fs::write(path, serde_json::to_string(value).expect("json")).expect("write");
}

#[test]
fn a_finished_job_reads_back_its_check_files_and_steps() {
    let root = scratch("finished");
    let video = root.join("a.mp4");
    std::fs::write(&video, b"video").expect("video");
    let work_root = root.join("work");
    let job = work_root.join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(&video).expect("c"),
    ));
    std::fs::create_dir_all(&job).expect("job");
    let mut steps = BTreeMap::new();
    for (step, wall_s) in [
        (StepName::ShotScan, 20.0),
        (StepName::Cues, 1.5),
        (StepName::ProbeDecode, 4.0),
    ] {
        steps.insert(
            step,
            StepRecord {
                fingerprint: String::new(),
                finished_ns: 0,
                measure: StepMeasure {
                    wall_s,
                    ..StepMeasure::default()
                },
            },
        );
    }
    write(
        &job.join("job.json"),
        &JobRecord {
            video: video.to_string_lossy().into_owned(),
            video_size: 5,
            video_modified_s: 0,
            settings: JobSettings::with_glossary(vec![]),
            models_dir: None,
            corrections: None,
            steps,
        },
    );
    let qc = QcReport {
        findings: vec![QcFinding {
            check: QcCheck::Unsure,
            time_s: 12.0,
            text: "Blaver!".into(),
            detail: "U0053".into(),
            utterance: Some("U0053".into()),
        }],
        ..QcReport::default()
    };
    write(&job.join("qc.json"), &qc);
    let report = load(&video, &work_root).expect("report");
    assert_eq!(report.qc, qc);
    assert_eq!(
        report.corrections,
        Corrections::default(),
        "no review.json yet"
    );
    assert_eq!(report.lines.to_check(), 1);
    assert!(report.problems.is_empty());
    assert_eq!(
        report.subtitles,
        std::fs::canonicalize(&video)
            .expect("c")
            .with_extension("srt")
    );
    let order: Vec<StepName> = report.steps.iter().map(|(s, _)| *s).collect();
    assert_eq!(
        order,
        [StepName::ProbeDecode, StepName::ShotScan, StepName::Cues]
    );
    assert_eq!(report.total_s(), 5.5, "the shot scan runs alongside");
    assert_eq!(
        summary(&video, &work_root),
        Ok(RowSummary {
            problems: 0,
            flagged: 1,
            to_check: 1,
            fixed_by_claude: false,
        })
    );
    assert_eq!(report.fix_result, None, "no fix.json yet");
    let corrections = Corrections {
        lines: vec![Correction {
            id: "U0053".into(),
            text: "Flavor!".into(),
            flags: Vec::new(),
            chosen: Chosen::Engine("W".into()),
        }],
    };
    write(&job.join("review.json"), &corrections);
    let report = load(&video, &work_root).expect("report");
    assert_eq!(report.corrections, corrections);
    assert_eq!((report.lines.flagged, report.lines.checked), (1, 1));
    assert_eq!(report.summary(), summary(&video, &work_root).expect("row"));
    assert_eq!(
        report.summary().to_check,
        0,
        "the corrected line is checked"
    );
    std::fs::write(job.join("review.json"), b"{").expect("broken");
    let error = summary(&video, &work_root).expect_err("broken review.json");
    assert!(error.contains("review.json"), "{error}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_current_fix_record_checks_the_lines_it_answered_and_a_stale_one_counts_for_nothing() {
    let root = scratch("fixed");
    let video = root.join("c.mp4");
    std::fs::write(&video, b"video").expect("video");
    let work_root = root.join("work");
    let job = work_root.join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(&video).expect("c"),
    ));
    std::fs::create_dir_all(&job).expect("job");
    let mut steps = BTreeMap::new();
    steps.insert(
        StepName::Readjudicate,
        StepRecord {
            fingerprint: "adjudication-1".into(),
            finished_ns: 0,
            measure: StepMeasure::default(),
        },
    );
    write(
        &job.join("job.json"),
        &JobRecord {
            video: video.to_string_lossy().into_owned(),
            video_size: 5,
            video_modified_s: 0,
            settings: JobSettings::with_glossary(vec![]),
            models_dir: None,
            corrections: None,
            steps,
        },
    );
    let finding = |id: &str| QcFinding {
        check: QcCheck::Unsure,
        time_s: 1.0,
        text: String::new(),
        detail: String::new(),
        utterance: Some(id.into()),
    };
    write(
        &job.join("qc.json"),
        &QcReport {
            findings: vec![finding("U1"), finding("U2")],
            ..QcReport::default()
        },
    );
    let line = LineFix {
        id: "U1".into(),
        problems: Vec::new(),
        checks: vec![QcCheck::Unsure],
        before_text: "Go!".into(),
        before_flags: Vec::new(),
        after_text: "Go!".into(),
        after_flags: Vec::new(),
        steps: Vec::new(),
        refused: Vec::new(),
        removed: Vec::new(),
        verdict: FixVerdict::Unchanged,
        applied: false,
    };
    let mut record = FixRecord {
        model: "opus".into(),
        lines: vec![line],
        adjudication: "adjudication-1".into(),
        ..FixRecord::default()
    };
    write(&job.join("fix.json"), &record);
    let report = load(&video, &work_root).expect("report");
    assert_eq!(
        (
            report.lines.flagged,
            report.lines.by_claude,
            report.lines.to_check()
        ),
        (2, 1, 1),
        "U1 was answered, U2 is left"
    );
    assert_eq!(report.fixable, 1, "Fix It asks about U2 alone");
    let result = report.fix_result.as_ref().expect("a result");
    assert_eq!(
        (result.model.as_str(), result.already_right),
        ("Claude Opus", 1)
    );
    let row = summary(&video, &work_root).expect("row");
    assert_eq!(row, report.summary());
    assert!(row.fixed_by_claude);
    record.adjudication = "adjudication-0".into();
    write(&job.join("fix.json"), &record);
    let stale = load(&video, &work_root).expect("report");
    assert_eq!(
        (stale.lines.by_claude, stale.fixable, stale.fix_result),
        (0, 2, None),
        "a record of an earlier re-adjudication counts for nothing"
    );
    assert!(!summary(&video, &work_root).expect("row").fixed_by_claude);
    std::fs::write(job.join("fix.json"), b"{").expect("broken");
    let error = summary(&video, &work_root).expect_err("broken fix.json");
    assert!(error.contains("fix.json"), "{error}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_job_without_a_check_names_the_missing_file() {
    let root = scratch("missing");
    let video = root.join("b.mp4");
    std::fs::write(&video, b"video").expect("video");
    let error = load(&video, &root.join("work")).expect_err("no job");
    assert!(error.contains("job.json"), "{error}");
    let _ = std::fs::remove_dir_all(&root);
}
