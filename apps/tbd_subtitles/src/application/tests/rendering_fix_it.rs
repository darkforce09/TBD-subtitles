//! Fix It on a finished job's Overview, rendered headless: the row and its button, the run under
//! way with Stop and its steps, the correction run that follows, the finish with its result card,
//! its toast and See Changes, the desktop told while the window is away, and the runs of the video
//! it holds meanwhile.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use job_model::outputs::{Correction, Corrections, FixBefore, FixRecord, FixVerdict, LineFix};
use job_model::report::QcCheck;
use pipeline::fix_it::{FixOutcome, FixProgress, FixStage};

use super::rendering_report::{check, finding, scratch, write_job};
use super::rendering_review::{work_dir, write_lines};
use super::*;
use crate::application::environment::NOTIFIED;
use crate::job_queue::models::queue::JobKind;
use crate::job_report::events::ReportEvent;
use crate::job_report::models::finding_group::LineGroup;
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

/// A window over `root` whose one finished video, Dressrosa 12, has its three lines (U1 unsure, U2
/// too short, U3 a heard word replaced) and speech with no subtitle; its Fix It runs through `fix`
/// and its runs through `run`.
fn flagged(root: &Path, fix: FixVideo, run: RunJob) -> (TbdSubtitlesApp, JobId) {
    let video = root.join("Dressrosa 12.mp4");
    std::fs::write(&video, b"video").expect("video");
    let mut env = Environment::scratch(root, run);
    env.fix_video = fix;
    let mut app = TbdSubtitlesApp::new(env, vec![video.clone()]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    let qc = check(vec![
        finding(QcCheck::Unsure, 10.0, Some("U1")),
        finding(QcCheck::TooShort, 12.0, Some("U2")),
        finding(QcCheck::RemovedLocked, 14.0, Some("U3")),
        finding(QcCheck::UncoveredSpeech, 20.0, None),
    ]);
    write_job(root, &video, &qc, None);
    write_lines(&work_dir(root, &video));
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    (app, id)
}

/// The work directory of `video`'s job under `work_root`.
fn job_dir(work_root: &Path, video: &Path) -> PathBuf {
    let video = std::fs::canonicalize(video).expect("the video");
    work_root.join(pipeline::work_dir::job_id(&video))
}

/// A run that, once the job's work directory exists, leaves a check that passes, as a correction
/// run that settles every finding does.
fn correcting() -> RunJob {
    Arc::new(|video, options, progress| {
        let job = job_dir(&options.work_root, video);
        if job.is_dir() {
            let passing = serde_json::to_string(&check(Vec::new())).expect("json");
            std::fs::write(job.join("qc.json"), passing).expect("qc.json");
        }
        stand_in()(video, options, progress)
    })
}

/// A run that fails once the job's work directory exists: a correction run that fails.
fn failing_update() -> RunJob {
    Arc::new(|video, options, progress| {
        if job_dir(&options.work_root, video).is_dir() {
            return Err(PipelineError::new("step review", "out of memory"));
        }
        stand_in()(video, options, progress)
    })
}

/// One line Fix It answered, `before` then `after`, with `verdict`.
fn fix_line(id: &str, before: &str, after: &str, verdict: FixVerdict) -> LineFix {
    LineFix {
        id: id.into(),
        problems: Vec::new(),
        checks: Vec::new(),
        before_text: before.into(),
        before_flags: Vec::new(),
        after_text: after.into(),
        after_flags: Vec::new(),
        steps: Vec::new(),
        refused: Vec::new(),
        removed: Vec::new(),
        applied: verdict.writes_correction(),
        verdict,
    }
}

/// A Fix It over the three lines of `flagged` that reads the video until `go` is set, then, when
/// `changes`, changes U1 and U3 and leaves U2, writing `review.json` and `fix.json` as the
/// pipeline does; otherwise it leaves every line as it was.
fn fixing(go: Arc<AtomicBool>, changes: bool) -> FixVideo {
    Arc::new(move |video, options, progress| {
        progress(FixProgress {
            stage: FixStage::Reading,
            done: 0,
            total: 1,
        });
        while !go.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(2));
        }
        let job = job_dir(&options.work_root, video);
        let qc = std::fs::read_to_string(job.join("qc.json")).expect("qc.json");
        let qc: QcReport = serde_json::from_str(&qc).expect("qc.json parses");
        let accepted = || FixVerdict::Accepted {
            why: "Both engines heard it.".into(),
        };
        let lines = if changes {
            vec![
                fix_line("U1", "Blaver!", "Flavor!", accepted()),
                fix_line("U2", "Go!", "Go!", FixVerdict::Unchanged),
                fix_line("U3", "Franky!", "Frankie!", accepted()),
            ]
        } else {
            ["U1", "U2", "U3"]
                .map(|id| fix_line(id, "Go!", "Go!", FixVerdict::Unchanged))
                .to_vec()
        };
        let changed: Vec<String> = lines
            .iter()
            .filter(|line| line.applied)
            .map(|line| line.id.clone())
            .collect();
        let corrections = Corrections {
            lines: lines
                .iter()
                .filter(|line| line.applied)
                .map(|line| Correction {
                    id: line.id.clone(),
                    text: line.after_text.clone(),
                    flags: Vec::new(),
                    chosen: job_model::outputs::Chosen::FixIt {
                        model: options.model.clone(),
                        why: line.why(),
                    },
                })
                .collect(),
        };
        let record = FixRecord {
            model: options.model.clone(),
            video: "Dressrosa 12".into(),
            lines,
            before: Some(FixBefore::of(&qc)),
            ..FixRecord::default()
        };
        if changes {
            let json = serde_json::to_string(&corrections).expect("json");
            std::fs::write(job.join("review.json"), json).expect("review.json");
        }
        let json = serde_json::to_string(&record).expect("json");
        std::fs::write(job.join("fix.json"), json).expect("fix.json");
        Ok(FixOutcome {
            work_dir: job,
            record,
            changed,
            kept_yours: Vec::new(),
        })
    })
}

/// The notifications the window sent, as "title: body".
fn notified(app: &TbdSubtitlesApp) -> Vec<String> {
    app.env
        .log
        .since(0)
        .into_iter()
        .filter(|line| line.target == NOTIFIED)
        .map(|line| line.message)
        .collect()
}

fn assert_shows(text: &str, expected: &[&str]) {
    for expected in expected {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
}

#[test]
fn fix_it_runs_on_the_video_and_queues_the_correction_run_that_times_its_changes() {
    let root = scratch("fix-it-runs");
    let go = Arc::new(AtomicBool::new(false));
    let (mut app, id) = finished_with(&root, "Dressrosa 12", waiting_fix(go.clone()));
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &[
            "Fix It with Claude Opus",
            "Claude Opus reads the whole video, fixes the flagged lines and checks each change.",
            "Fix It",
        ],
    );
    app.apply(vec![Action::from(ReportEvent::FixIt)]);
    until_reading(&mut app);
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &[
            "Fixing with Claude Opus · reading the whole video (1 of 4)…",
            "Stop",
            "Fixing with Claude · 1 of 4",
        ],
    );
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
    assert_shows(
        &text,
        &[
            "Fixing with Claude Opus · updating the subtitles (4 of 4)…",
            "Fixing with Claude · 4 of 4",
        ],
    );
    assert!(
        !text.contains("is fixed"),
        "no toast before the run: {text}"
    );
    assert!(
        !text.contains("Updating subtitles with"),
        "Fix It's note stands in for the correction note: {text}"
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
fn fix_it_finishes_once_its_changes_are_in_the_subtitles() {
    let root = scratch("fix-it-done");
    let go = Arc::new(AtomicBool::new(false));
    let (mut app, id) = flagged(&root, fixing(go.clone(), true), correcting());
    app.presence.away = true;
    app.apply(vec![Action::from(ReportEvent::FixIt)]);
    until_reading(&mut app);
    go.store(true, Ordering::SeqCst);
    until_fixed(&mut app);
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &["Fixing with Claude Opus · updating the subtitles (4 of 4)…"],
    );
    assert!(!text.contains("Fixed by"), "{text}");
    assert!(notified(&app).is_empty(), "nothing is finished yet");
    settle(&mut app);
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &[
            "Fixed by Claude Opus",
            "2 lines changed · 1 was already right",
            "1 subtitle broke a layout rule",
            "Speech with no subtitle",
            "“Blaver!” → “Flavor!”",
            "“Franky!” → “Frankie!”",
            "See Changes",
            "Dressrosa 12 is fixed: Claude changed 2 lines. The subtitles are ready.",
            "Passes the quality check",
            "All 2 lines checked",
            "Claude checked 2",
            "Subtitles ready · fixed by Claude",
        ],
    );
    assert!(!text.contains("Claude could not fix"), "{text}");
    assert!(!text.contains("Fixing with"), "{text}");
    assert_eq!(
        notified(&app),
        ["Dressrosa 12 is fixed: Claude changed 2 lines. The subtitles are ready."],
        "the desktop is told once, while the window is away"
    );
    assert!(app.attention, "the window asks for the desktop's attention");
    assert!(app.fix_followups.is_empty());
    // See Changes in the toast opens Check Lines on Claude's changes.
    let toast = app
        .toasts
        .shown()
        .iter()
        .find(|toast| toast.action.is_some())
        .map(|toast| toast.id)
        .expect("the toast offers See Changes");
    app.apply(vec![Action::ToastButton(toast)]);
    let group = app
        .review
        .as_ref()
        .map(|(job, session)| (*job, session.group));
    assert_eq!(group, Some((id, Some(LineGroup::ChangedByFixIt))));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fix_it_with_nothing_to_change_finishes_at_once_and_leaves_the_desktop_alone_in_front() {
    let root = scratch("fix-it-nothing");
    let (mut app, _) = flagged(
        &root,
        fixing(Arc::new(AtomicBool::new(true)), false),
        correcting(),
    );
    app.apply(vec![Action::from(ReportEvent::FixIt)]);
    until_fixed(&mut app);
    assert!(
        app.queue
            .items
            .iter()
            .all(|item| item.kind == JobKind::Full),
        "no correction run"
    );
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &[
            "Claude checked Dressrosa 12: every line was already right.",
            "Fixed by Claude Opus",
            "0 lines changed · 3 were already right",
            "Claude could not fix: 1 subtitle breaks a layout rule",
            "Claude could not fix: Speech with no subtitle",
            "Show Nearby Lines",
        ],
    );
    assert!(!text.contains("See Changes"), "nothing to see: {text}");
    assert!(notified(&app).is_empty(), "the window is in front");
    assert!(!app.attention);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_failed_correction_run_finishes_nothing() {
    let root = scratch("fix-it-failed-update");
    let (mut app, _) = flagged(
        &root,
        fixing(Arc::new(AtomicBool::new(true)), true),
        failing_update(),
    );
    app.presence.away = true;
    app.apply(vec![Action::from(ReportEvent::FixIt)]);
    until_fixed(&mut app);
    settle(&mut app);
    assert!(app.fix_followups.is_empty(), "the run is forgotten");
    let (text, _) = render(&app);
    assert!(!text.contains("is fixed"), "{text}");
    assert!(!text.contains("Fixing with"), "{text}");
    assert!(notified(&app).is_empty());
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
    assert!(app.review_lanes.hold(id, pipeline::CancelToken::new()));
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
