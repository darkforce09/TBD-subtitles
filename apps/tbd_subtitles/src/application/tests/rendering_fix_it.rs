//! Fix It on a finished job's Overview, rendered headless: the row and its button, the run under
//! way with Stop, what happens when it ends, and the runs of the video it holds meanwhile.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use job_model::outputs::FixRecord;
use job_model::report::QcCheck;
use pipeline::fix_it::{FixOutcome, FixProgress, FixStage};

use super::rendering_report::{check, finding, scratch, write_job};
use super::*;
use crate::job_queue::models::queue::JobKind;
use crate::job_report::events::ReportEvent;
use crate::job_report::services::fix_it::FixVideo;

/// A window over `root` whose one finished video, `name`, has a line too short and an unsure
/// line, and whose Fix It runs through `fix`.
fn finished_with(root: &Path, name: &str, fix: FixVideo) -> (TbdSubtitlesApp, JobId) {
    let video = root.join(format!("{name}.mp4"));
    std::fs::write(&video, b"video").expect("video");
    let mut env = Environment::scratch(root, stand_in());
    env.fix_video = fix;
    let mut app = TbdSubtitlesApp::new(env, vec![video.clone()]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    let qc = check(vec![
        finding(QcCheck::TooShort, 1156.1, Some("U0314")),
        finding(QcCheck::Unsure, 246.8, Some("U0053")),
    ]);
    write_job(root, &video, &qc, None);
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    (app, id)
}

/// A Fix It that says it reads the video, waits until `go` is set or it is stopped, then changes
/// line U0314.
fn waiting_fix(go: Arc<AtomicBool>) -> FixVideo {
    Arc::new(move |video, options, progress| {
        progress(FixProgress {
            stage: FixStage::Reading,
            done: 0,
            total: 1,
        });
        while !go.load(Ordering::SeqCst) {
            if options.cancel.is_cancelled() {
                return Err(PipelineError::cancelled("Fix It"));
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        Ok(FixOutcome {
            work_dir: video.to_path_buf(),
            record: FixRecord {
                model: options.model.clone(),
                ..FixRecord::default()
            },
            changed: vec!["U0314".into()],
            kept_yours: Vec::new(),
        })
    })
}

/// Poll until the Fix It run ended, or fail after five seconds.
fn until_fixed(app: &mut TbdSubtitlesApp) {
    for _ in 0..500 {
        app.poll();
        if app.pending.fix.is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the Fix It run never ended");
}

/// Poll until the run has said which pass it is in.
fn until_reading(app: &mut TbdSubtitlesApp) {
    for _ in 0..500 {
        app.poll();
        if app
            .pending
            .fix
            .as_ref()
            .is_some_and(|(_, fixing)| fixing.progress.is_some())
        {
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("the Fix It run never reported");
}

#[test]
fn fix_it_runs_on_the_video_and_queues_the_correction_run_that_times_its_changes() {
    let root = scratch("fix-it-runs");
    let go = Arc::new(AtomicBool::new(false));
    let (mut app, id) = finished_with(&root, "Dressrosa 12", waiting_fix(go.clone()));
    let (text, _) = render(&app);
    for expected in [
        "Fix It with Claude Opus",
        "Claude Opus reads the whole video, fixes the flagged lines and checks each change.",
        "Fix It",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    app.apply(vec![Action::from(ReportEvent::FixIt)]);
    until_reading(&mut app);
    let (text, _) = render(&app);
    assert!(
        text.contains("Fixing with Claude Opus · reading the whole video (1 of 3)…"),
        "{text}"
    );
    assert!(text.contains("Stop"), "{text}");
    assert!(!text.contains("Fix It with Claude Opus"), "{text}");
    // Nothing of the video runs meanwhile: a correction run waits.
    let video = app.queue.items[0].video.clone();
    queue_editing::queue_review(&mut app.queue, video, 1);
    app.start_next();
    let review = |app: &TbdSubtitlesApp| {
        app.queue
            .items
            .iter()
            .find(|item| item.kind == JobKind::Review)
            .map(|item| item.state.clone())
    };
    assert_eq!(review(&app), Some(JobState::Waiting));
    // Nor can its row be removed.
    app.apply(vec![Action::from(JobQueueEvent::Remove(id))]);
    assert!(app.queue.get(id).is_some());
    go.store(true, Ordering::SeqCst);
    until_fixed(&mut app);
    let carried = app
        .queue
        .items
        .iter()
        .find(|item| item.kind == JobKind::Review)
        .map(|item| item.corrections);
    assert_eq!(
        carried,
        Some(2),
        "the waiting run carries Fix It's change too"
    );
    let (text, _) = render(&app);
    assert!(
        text.contains("Claude Opus changed 1 line in Dressrosa 12. Updating the subtitles."),
        "{text}"
    );
    assert!(
        app.parked
            .get(&id)
            .is_some_and(|parked| parked.runs.iter().any(|(line, _)| line == "U0314")),
        "the changed line waits for its run"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn stop_ends_the_run_and_changes_nothing() {
    let root = scratch("fix-it-stop");
    let (mut app, _) = finished_with(
        &root,
        "Dressrosa 12",
        waiting_fix(Arc::new(AtomicBool::new(false))),
    );
    app.apply(vec![Action::from(ReportEvent::FixIt)]);
    until_reading(&mut app);
    app.apply(vec![Action::from(ReportEvent::StopFix)]);
    let (text, _) = render(&app);
    assert!(text.contains("Stopping…"), "{text}");
    until_fixed(&mut app);
    assert!(
        app.queue
            .items
            .iter()
            .all(|item| item.kind == JobKind::Full)
    );
    let (text, _) = render(&app);
    assert!(
        text.contains(
            "Fix It stopped. Nothing was changed; Fix It again picks up where it stopped."
        ),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fix_it_is_off_while_the_video_s_subtitles_are_updated_and_hidden_with_nothing_to_fix() {
    let root = scratch("fix-it-off");
    let (mut app, id) = finished_with(
        &root,
        "Dressrosa 12",
        waiting_fix(Arc::new(AtomicBool::new(true))),
    );
    app.queue.running = false;
    app.review_cancel = Some((id, pipeline::CancelToken::new()));
    let video = app.queue.items[0].video.clone();
    queue_editing::queue_review(&mut app.queue, video.clone(), 1);
    let (text, _) = render(&app);
    assert!(
        text.contains("Wait until this video's subtitles are updated."),
        "{text}"
    );
    app.apply(vec![Action::from(ReportEvent::FixIt)]);
    assert!(app.pending.fix.is_none());
    write_job(
        &root,
        &video,
        &check(vec![finding(QcCheck::Offset, 0.0, None)]),
        None,
    );
    app.refresh_report(true);
    let (text, _) = render(&app);
    assert!(!text.contains("Fix It with"), "{text}");
    let _ = std::fs::remove_dir_all(&root);
}
