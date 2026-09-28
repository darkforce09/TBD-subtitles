use std::collections::BTreeMap;

use job_model::job::{JobSettings, StepMeasure, StepRecord};
use job_model::outputs::{Chosen, Correction};
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
        })
    );
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
fn a_job_without_a_check_names_the_missing_file() {
    let root = scratch("missing");
    let video = root.join("b.mp4");
    std::fs::write(&video, b"video").expect("video");
    let error = load(&video, &root.join("work")).expect_err("no job");
    assert!(error.contains("job.json"), "{error}");
    let _ = std::fs::remove_dir_all(&root);
}
