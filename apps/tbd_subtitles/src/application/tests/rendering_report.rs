//! A finished job's Overview and its row rendered headless: the file card, the lines card, the
//! disclosures, the sidebar's verdict and count, and Try Again for a failed language-model call.

use std::path::Path;

use job_model::outputs::{Chosen, Correction, Corrections};
use job_model::report::{QcCheck, QcFinding, QcSummary};

use super::*;
use crate::job_report::events::ReportEvent;

/// A scratch folder of its own for test `name`.
pub(super) fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("tbd-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    root
}

/// A window over `root` whose one video, `name`, the stand-in runner has finished.
fn finished(root: &Path, name: &str) -> (TbdSubtitlesApp, PathBuf) {
    let video = root.join(format!("{name}.mp4"));
    std::fs::write(&video, b"video").expect("video");
    let mut app = TbdSubtitlesApp::new(Environment::scratch(root, stand_in()), vec![video.clone()]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    (app, video)
}

/// What the pipeline leaves in `video`'s work directory under `root`: `job.json`, `qc.json`, and
/// the owner's `review.json` when there are `corrections`.
pub(super) fn write_job(
    root: &Path,
    video: &Path,
    qc: &QcReport,
    corrections: Option<&Corrections>,
) {
    let job = root.join("work").join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(video).expect("c"),
    ));
    std::fs::create_dir_all(&job).expect("job");
    let record = job_model::job::JobRecord {
        video: video.to_string_lossy().into_owned(),
        video_size: 5,
        video_modified_s: 0,
        settings: job_model::job::JobSettings {
            onscreen_text: job_model::onscreen::TextSettings::default(),
            ..job_model::job::JobSettings::with_glossary(vec![])
        },
        models_dir: None,
        corrections: None,
        steps: Default::default(),
    };
    std::fs::write(job.join("job.json"), json(&record)).expect("job.json");
    std::fs::write(job.join("qc.json"), json(qc)).expect("qc.json");
    if let Some(corrections) = corrections {
        std::fs::write(job.join("review.json"), json(corrections)).expect("review.json");
    }
}

fn json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).expect("json")
}

pub(super) fn finding(check: QcCheck, time_s: f64, line: Option<&str>) -> QcFinding {
    QcFinding {
        check,
        time_s,
        text: "Blaver!".into(),
        detail: String::new(),
        utterance: line.map(str::to_string),
    }
}

/// A check with ten cues, all easy to read, and `findings`.
pub(super) fn check(findings: Vec<QcFinding>) -> QcReport {
    QcReport {
        summary: QcSummary {
            cues: 10,
            dialogue_cues: 9,
            sound_cues: 1,
            cps_ok_share: 1.0,
            ..QcSummary::default()
        },
        findings,
    }
}

#[test]
fn a_finished_job_shows_its_report() {
    let root = scratch("report");
    let (mut app, video) = finished(&root, "Dressrosa 12");
    let qc = check(vec![finding(QcCheck::Unsure, 246.8, Some("U0053"))]);
    write_job(&root, &video, &qc, None);
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, actions) = render(&app);
    for expected in [
        "Dressrosa 12",
        "0:00 video · finished in 0 s",
        "Overview",
        "Check Lines",
        "Subtitles saved next to the video",
        "Passes the quality check",
        "Open in Player",
        "Show in Folder",
        "Copy Path",
        "1 line worth a listen",
        "0 of 1 checked",
        "Unsure what was said",
        "Details",
        "10 subtitles · 100.0 % easy to read",
        "Step times",
        "0 s in total, shot scan aside",
        "Subtitles ready · 1 to check",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(
        !text.contains("Easy to read") && !text.contains("Open Full Report"),
        "the disclosures start closed: {text}"
    );
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    // The work folder has no lines to check.
    app.apply(vec![Action::ShowTab(DetailTab::CheckLines)]);
    assert_eq!(app.detail_tab(id), DetailTab::Overview);
    assert!(
        matches!(app.report, Some((_, Ok(_)))),
        "the report stays: {:?}",
        app.report
    );
    let (text, _) = render(&app);
    assert!(text.contains("Check Lines cannot open"), "{text}");
    assert!(text.contains("Unsure what was said"), "{text}");
    app.apply(vec![Action::from(ReportEvent::Copied)]);
    let (text, _) = render(&app);
    assert!(text.contains("Subtitle path copied"), "{text}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failed_language_model_call_needs_attention_and_try_again_reruns_the_calls() {
    let root = scratch("attention");
    let (mut app, video) = finished(&root, "Dressrosa 14");
    let qc = check(vec![
        finding(QcCheck::FailedCall, 0.0, None),
        finding(QcCheck::TooFast, 12.0, Some("U0007")),
    ]);
    write_job(&root, &video, &qc, None);
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, actions) = render(&app);
    for expected in [
        "Needs attention",
        "The file holds the app's best reading of every line, but the quality check found \
         problems.",
        "1 language-model call failed",
        "Some lines may be missing. Try Again runs that step and the ones after it.",
        "Try Again",
        "Too fast to read",
        "Needs attention · 1 problem",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(!text.contains("Passes the quality check"), "{text}");
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    app.apply(vec![Action::from(ReportEvent::TryAgain)]);
    let item = app.queue.get(id).expect("the job");
    assert!(item.state.is_running(), "it starts at once: {item:?}");
    assert_eq!(item.rerun, [StepName::Adjudicate], "from the model calls");
    let (text, _) = render(&app);
    assert!(
        text.contains("Trying Dressrosa 14 again from Settle the words."),
        "{text}"
    );
    settle(&mut app);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_job_finished_in_an_earlier_window_shows_its_verdict_and_lines_to_check() {
    let root = scratch("earlier");
    let video = root.join("Dressrosa 15.mp4");
    std::fs::write(&video, b"video").expect("video");
    let mut first =
        TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), vec![video.clone()]);
    first.queue.items[0].state = JobState::FinishedBefore;
    first.save_queue();
    drop(first);
    let qc = check(vec![
        finding(QcCheck::Unsure, 1.0, Some("U1")),
        finding(QcCheck::TooFast, 2.0, Some("U2")),
        finding(QcCheck::RemovedLocked, 3.0, Some("U3")),
    ]);
    let corrections = Corrections {
        lines: vec![Correction {
            id: "U1".into(),
            text: "Flavor!".into(),
            flags: Vec::new(),
            chosen: Chosen::Typed,
        }],
    };
    write_job(&root, &video, &qc, Some(&corrections));
    let mut app = TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), Vec::new());
    let (text, _) = render(&app);
    assert!(
        text.contains("Subtitles ready · 2 to check"),
        "its real count, read when the window opens: {text}"
    );
    assert!(
        text.lines().any(|line| line == "2"),
        "the orange count at the row's end: {text}"
    );
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, _) = render(&app);
    for expected in [
        "3 lines worth a listen",
        "1 of 3 checked",
        "Unsure what was said",
        "Too fast to read",
        "Heard word replaced",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn show_nearby_lines_opens_every_line_at_the_one_nearest_the_speech_with_no_subtitle() {
    let root = scratch("nearby");
    let (mut app, video) = finished(&root, "Dressrosa 18");
    let qc = check(vec![finding(QcCheck::UncoveredSpeech, 20.0, None)]);
    write_job(&root, &video, &qc, None);
    let job = root.join("work").join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(&video).expect("c"),
    ));
    let line = |id: &str, start: f64| {
        format!(
            r#"{{"id":"{id}","start_s":{start},"end_s":{},"words":[],"locked":[],"line":"{id}","hypotheses":[["P",["hey"]]]}}"#,
            start + 1.0
        )
    };
    let sheet = format!("[{},{}]", line("U1", 10.0), line("U2", 25.0));
    let adjudicated = r#"{"lines":[{"id":"U1","t":"Hey.","f":[]},{"id":"U2","t":"Hey!","f":[]}],
        "findings":{"missing_ids":[],"duplicate_ids":[],"unknown_ids":[],"novel":[],"removed_locked":[],"too_fast":[]},
        "calls":1,"input_tokens":0,"output_tokens":0,"cost_usd":0.0}"#;
    std::fs::write(job.join("sheet.json"), sheet).expect("sheet");
    std::fs::write(job.join("adjudicated.json"), adjudicated).expect("adjudicated");
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, _) = render(&app);
    assert!(text.contains("Show Nearby Lines"), "{text}");
    let at = match app.report.as_ref().map(|(_, report)| report) {
        Some(Ok(report)) => report.problems.first().and_then(|p| p.remedy()),
        _ => None,
    };
    let Some(crate::job_report::models::problem::Remedy::ShowNearbyLines(at)) = at else {
        panic!("the problem offers Show Nearby Lines: {at:?}");
    };
    app.apply(vec![Action::from(ReportEvent::CheckLines(
        crate::job_report::events::LinesToCheck::Near(at),
    ))]);
    let (_, session) = app.review.as_ref().expect("Check Lines is open");
    assert_eq!(
        session.list,
        crate::line_review::models::session::LineList::All,
        "every line shows"
    );
    assert_eq!(
        session.open.as_deref(),
        Some("U2"),
        "the line nearest 20 s: U2 starts 5 s later, U1 ended 9 s before"
    );
    let _ = std::fs::remove_dir_all(&root);
}
