use std::collections::BTreeMap;

use job_model::job::{JobSettings, StepMeasure, StepRecord};
use job_model::outputs::{Chosen, Correction, FixVerdict, LineFix};
use job_model::report::{QcCheck, QcFinding};

use super::*;
use crate::job_report::models::report::LocalizedOutput;

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
            settings: JobSettings {
                onscreen_text: job_model::onscreen::TextSettings::default(),
                ..JobSettings::with_glossary(vec![])
            },
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
            fixable: 1,
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
            settings: JobSettings {
                onscreen_text: job_model::onscreen::TextSettings::default(),
                ..JobSettings::with_glossary(vec![])
            },
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

/// A finished job of `video` under `work_root` with on-screen text on and the localized video
/// `localized`, having run `steps`; its work directory.
fn visual_job(video: &Path, work_root: &Path, localized: bool, steps: &[StepName]) -> PathBuf {
    let job = work_root.join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(video).expect("c"),
    ));
    std::fs::create_dir_all(job.join("visual")).expect("job");
    let mut settings = JobSettings::with_glossary(vec![]);
    settings.onscreen_text.enabled = true;
    settings.onscreen_text.localized_video = localized;
    let record = |step: &StepName| {
        let done = StepRecord {
            fingerprint: String::new(),
            finished_ns: 0,
            measure: StepMeasure::default(),
        };
        (*step, done)
    };
    write(
        &job.join("job.json"),
        &JobRecord {
            video: video.to_string_lossy().into_owned(),
            video_size: 5,
            video_modified_s: 0,
            settings,
            models_dir: None,
            corrections: None,
            steps: steps.iter().map(record).collect(),
        },
    );
    write(&job.join("qc.json"), &QcReport::default());
    write(
        &job.join("visual/text_typeset.json"),
        &job_model::onscreen::TextDocument::default(),
    );
    job
}

/// A replacement document with `baked` occurrences drawn in and one left out.
fn composed(baked: usize) -> ReplacementDocument {
    use job_model::onscreen::{ReplaceStatus, ReplacedText};
    let text = |id: String, status: ReplaceStatus| ReplacedText {
        id,
        first_frame: 0,
        last_frame: 1,
        status,
        style: None,
        container: None,
        plates: Vec::new(),
        preview: None,
        lettering_quad: None,
    };
    let mut texts: Vec<ReplacedText> = (0..baked)
        .map(|n| text(format!("T{n}"), ReplaceStatus::Baked))
        .collect();
    texts.push(text(
        "T9".into(),
        ReplaceStatus::Fallback("too busy".into()),
    ));
    ReplacementDocument {
        width: 1920,
        height: 1080,
        frame_count: 10,
        texts,
    }
}

#[test]
fn a_job_without_the_localized_video_reports_none() {
    let root = scratch("no-localized");
    let video = root.join("a.mp4");
    std::fs::write(&video, b"video").expect("video");
    let work_root = root.join("work");
    let job = visual_job(&video, &work_root, false, &[StepName::LocalizedVideo]);
    write(
        &job.join("visual/localized_video.json"),
        &LocalizedVideoRecord::default(),
    );
    let report = load(&video, &work_root).expect("report");
    assert_eq!(report.localized, None);
    assert!(report.visual.is_some());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_localized_video_reports_what_it_replaced_and_its_files_while_they_are_there() {
    let root = scratch("localized");
    let video = root.join("a.mp4");
    std::fs::write(&video, b"video").expect("video");
    let work_root = root.join("work");
    visual_job(&video, &work_root, true, &[]);
    let report = load(&video, &work_root).expect("no records are no error");
    assert_eq!(report.localized, Some(LocalizedOutput::default()));

    let job = visual_job(&video, &work_root, true, &[StepName::TextCompose]);
    write(&job.join("visual/text_compose.json"), &composed(3));
    let report = load(&video, &work_root).expect("report");
    let localized = report.localized.expect("localized");
    assert_eq!(localized.replaced, Some(3), "counted from the composition");
    assert_eq!(localized.video, None, "not written yet");

    let steps = [StepName::TextCompose, StepName::TextVerify];
    let job = visual_job(&video, &work_root, true, &steps);
    write(&job.join("visual/text_verify.json"), &composed(1));
    let report = load(&video, &work_root).expect("report");
    assert_eq!(
        report.localized.and_then(|l| l.replaced),
        Some(1),
        "counted from what the read-back check approved"
    );

    let steps = [StepName::TextCompose, StepName::LocalizedVideo];
    let job = visual_job(&video, &work_root, true, &steps);
    let mkv = root.join("a.localized.mkv");
    let ass = root.join("a.localized.ass");
    write(
        &job.join("visual/localized_video.json"),
        &LocalizedVideoRecord {
            path: Some(mkv.display().to_string()),
            frames: 10,
            replaced: 2,
            ..LocalizedVideoRecord::default()
        },
    );
    write(
        &job.join("output.json"),
        &OutputRecord {
            path: root.join("a.ass").display().to_string(),
            localized: Some(ass.display().to_string()),
            ..OutputRecord::default()
        },
    );
    let report = load(&video, &work_root).expect("report");
    let localized = report.localized.clone().expect("localized");
    assert_eq!(localized.replaced, Some(2), "the video's own count");
    assert_eq!(
        (localized.video, localized.subtitles),
        (None, None),
        "no files"
    );
    std::fs::write(&mkv, b"mkv").expect("mkv");
    std::fs::write(&ass, b"ass").expect("ass");
    let localized = load(&video, &work_root).expect("report").localized;
    assert_eq!(
        localized,
        Some(LocalizedOutput {
            replaced: Some(2),
            video: Some(mkv),
            subtitles: Some(ass),
        })
    );
    std::fs::write(job.join("visual/localized_video.json"), b"not json").expect("broken");
    let report = load(&video, &work_root).expect("a broken record is no error");
    assert_eq!(report.localized.expect("localized").video, None);
    let _ = std::fs::remove_dir_all(&root);
}
