use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use job_model::StepName;
use job_model::report::QcReport;
use pipeline::progress::Progress;
use pipeline::{JobOutcome, PipelineError};

use super::*;
use crate::core::color_scheme::Scheme;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::progress::{FinishedStep, JobProgress, StepState};
use crate::job_queue::models::queue::{Failure, JobState};
use crate::job_queue::services::job_runner::RunJob;
use crate::job_queue::services::queue_editing;
use crate::settings::events::SettingsEvent;

#[path = "rendering_automation.rs"]
mod rendering_automation;
#[path = "rendering_console.rs"]
mod rendering_console;
#[path = "rendering_detail.rs"]
mod rendering_detail;
#[path = "rendering_fix_it.rs"]
mod rendering_fix_it;
#[path = "rendering_fix_many.rs"]
mod rendering_fix_many;
#[path = "rendering_queue.rs"]
mod rendering_queue;
#[path = "rendering_report.rs"]
mod rendering_report;
#[path = "rendering_review.rs"]
mod rendering_review;
#[path = "rendering_settings.rs"]
mod rendering_settings;
#[path = "rendering_text.rs"]
mod rendering_text;
#[path = "window_snapshots.rs"]
mod window_snapshots;

/// A stand-in for the pipeline: one step, then success, or cancelled when the token is set.
fn stand_in() -> RunJob {
    Arc::new(|video, options, progress| {
        progress(Progress::JobDuration(60.0));
        progress(Progress::StepStarted(StepName::Cues));
        if options.cancel.is_cancelled() {
            return Err(PipelineError::cancelled("step cues"));
        }
        Ok(JobOutcome {
            work_dir: options.work_root.join("job"),
            subtitles: video.with_extension("srt"),
            report: options.work_root.join("job").join("report.md"),
            qc: QcReport::default(),
            ran: vec![StepName::Cues],
            skipped: Vec::new(),
        })
    })
}

/// A stand-in whose Whisper step fails after two steps still valid; it keeps the steps it was
/// asked to run again in `rerun`.
fn failing(rerun: Arc<Mutex<Vec<StepName>>>) -> RunJob {
    Arc::new(move |_video, options, progress| {
        if let Ok(mut seen) = rerun.lock() {
            seen.clone_from(&options.rerun);
        }
        progress(Progress::StepSkipped(StepName::ProbeDecode));
        progress(Progress::StepSkipped(StepName::ShotScan));
        progress(Progress::StepStarted(StepName::AsrWhisper));
        progress(Progress::StepFailed {
            step: StepName::AsrWhisper,
            message: "step asr_whisper: out of memory".into(),
        });
        Err(PipelineError::new("step asr_whisper", "out of memory"))
    })
}

/// A stand-in that fails before its job starts, as when another process holds the job's lock.
fn locked() -> RunJob {
    Arc::new(|_video, _options, _progress| {
        Err(PipelineError::new(
            "lock the work directory",
            "another process runs this job",
        ))
    })
}

/// A stand-in that keeps two steps still valid, then runs its separation until it is cancelled.
fn until_cancelled() -> RunJob {
    Arc::new(|_video, options, progress| {
        progress(Progress::StepSkipped(StepName::ProbeDecode));
        progress(Progress::StepSkipped(StepName::ShotScan));
        progress(Progress::StepStarted(StepName::Separation));
        for _ in 0..500 {
            if options.cancel.is_cancelled() {
                progress(Progress::StepFailed {
                    step: StepName::Separation,
                    message: "step separation: cancelled".into(),
                });
                return Err(PipelineError::cancelled("step separation"));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(PipelineError::new("step separation", "never cancelled"))
    })
}

/// An application over files in a scratch folder of its own, never the owner's home.
fn app(name: &str, videos: Vec<PathBuf>) -> TbdSubtitlesApp {
    app_with(name, videos, stand_in())
}

/// An application whose jobs run through `run`, in a scratch folder of its own.
fn app_with(name: &str, videos: Vec<PathBuf>, run: RunJob) -> TbdSubtitlesApp {
    let root = std::env::temp_dir().join(format!("tbd-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    TbdSubtitlesApp::new(Environment::scratch(&root, run), videos)
}

/// Poll the runner until nothing runs, or fail after five seconds.
fn settle(app: &mut TbdSubtitlesApp) {
    for _ in 0..500 {
        app.poll();
        if app.queue.running_job().is_none() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the queue did not settle");
}

/// Run two headless frames in the window's theme and return every piece of text painted, with
/// the actions asked for.
fn render(app: &TbdSubtitlesApp) -> (String, Vec<Action>) {
    render_with(app, Vec::new())
}

/// As `render`, with `events` (keys pressed, say) given to the first frame.
fn render_with(app: &TbdSubtitlesApp, mut events: Vec<egui::Event>) -> (String, Vec<Action>) {
    let context = egui::Context::default();
    theme::install(&context);
    theme::follow(&context, app.scheme);
    let mut text = String::new();
    let mut actions = Vec::new();
    // Panels and windows settle their sizes on the first frame; the text is the second's.
    for _ in 0..2 {
        text.clear();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1800.0),
            )),
            events: std::mem::take(&mut events),
            ..Default::default()
        };
        let mut output = context.run_ui(input, |ui| actions.extend(app.frame_ui(ui)));
        output.textures_delta.clear();
        for clipped in output.shapes {
            if let egui::epaint::Shape::Text(shape) = &clipped.shape {
                text.push_str(shape.galley.text());
                text.push('\n');
            }
        }
    }
    (text, actions)
}

/// A job ten minutes into a 25:59 video, its words being settled: every step before done, the
/// language model 40 s into its fourth batch of eight, the rest to run.
fn settling(now: Instant) -> JobProgress {
    let mut progress = JobProgress::new(now - Duration::from_secs(600));
    progress.duration_s = Some(1559.0);
    for row in &mut progress.steps {
        row.state = match row.step {
            StepName::Adjudicate => StepState::Running {
                started: now - Duration::from_secs(40),
                done: 3,
                total: 8,
                message: Some("batch 4 of 8".into()),
            },
            step if step < StepName::Adjudicate => StepState::Done { wall_s: 30.0 },
            _ => StepState::Pending,
        };
    }
    progress
}

fn videos(app: &TbdSubtitlesApp) -> Vec<PathBuf> {
    app.queue.items.iter().map(|i| i.video.clone()).collect()
}

#[test]
fn an_empty_queue_says_how_to_add_videos() {
    let (text, actions) = render(&app("empty", Vec::new()));
    for expected in [
        "Add Videos…",
        "Add Folder…",
        "Start Queue",
        "Download the models first",
        "No videos yet",
        "Drop videos here",
        "The models download first; you can add videos now.",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(actions.is_empty(), "an idle frame asks for nothing");
}

#[test]
fn queued_videos_show_by_name_and_wait_for_start() {
    let app = app("names", vec![PathBuf::from("/videos/Dressrosa 08.mp4")]);
    let (text, _) = render(&app);
    for expected in [
        "UP NEXT",
        "Dressrosa 08",
        "Waiting · next in line",
        "Start Queue",
        "Download the models first",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(!text.contains("No videos yet"), "{text}");
}

#[test]
fn actions_change_the_queue_only_when_applied() {
    let mut app = app(
        "apply",
        vec![PathBuf::from("a.mp4"), PathBuf::from("b.mp4")],
    );
    app.apply(vec![
        Action::from(JobQueueEvent::Remove(0)),
        Action::QueueVideos(vec![PathBuf::from("b.mp4"), PathBuf::from("c.mp4")]),
    ]);
    assert_eq!(
        videos(&app),
        [PathBuf::from("b.mp4"), PathBuf::from("c.mp4")]
    );
}

#[test]
fn no_job_starts_while_a_model_is_missing() {
    let mut app = app("missing", vec![PathBuf::from("a.mp4")]);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    assert!(app.queue.items[0].state.is_waiting());
}

#[test]
fn started_jobs_run_one_after_another_and_the_queue_is_kept() {
    let mut app = app("run", vec![PathBuf::from("a.mp4"), PathBuf::from("b.mp4")]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    assert!(app.queue.items[0].state.is_running());
    assert!(app.queue.items[1].state.is_waiting(), "one job at a time");
    settle(&mut app);
    for item in &app.queue.items {
        assert!(matches!(item.state, JobState::Finished(_)), "{item:?}");
    }
    let done: Vec<JobId> = crate::job_queue::services::sidebar_rows::rows(&app.queue)
        .iter()
        .map(|row| row.id)
        .collect();
    assert_eq!(done, [1, 0], "the newest finished first");
    assert!(!app.queue.running, "an empty queue stops");
    let kept = crate::job_queue::services::queue_store::load(&app.env.queue_path).expect("kept");
    assert_eq!(kept.items.len(), 2);
}

#[test]
fn a_cancelled_job_keeps_its_finished_steps_and_can_be_retried() {
    let mut app = app_with("cancel", vec![PathBuf::from("a.mp4")], until_cancelled());
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Cancel(id))]);
    settle(&mut app);
    assert_eq!(
        app.queue.items[0].state,
        JobState::Cancelled { kept_steps: 2 }
    );
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, actions) = render(&app);
    for expected in [
        "Cancelled",
        "Cancelled · 2 finished steps kept",
        "2 finished steps are kept. Try Again continues after them. It starts at once.",
        "Try Again",
        "Remove from List",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    app.apply(vec![
        Action::from(JobQueueEvent::Pause),
        Action::from(JobQueueEvent::TryAgain(id, None)),
    ]);
    assert!(
        app.queue.items[0].state.is_running(),
        "Try Again starts at once when nothing runs"
    );
    assert!(!app.queue.running, "without turning the queue on");
    assert!(
        app.queue.items[0].keep_settings,
        "a job tried again keeps its own settings"
    );
    let (text, _) = render(&app);
    assert!(
        text.contains("Trying a again from Separate the voices."),
        "{text}"
    );
    app.apply(vec![Action::from(JobQueueEvent::Cancel(id))]);
    settle(&mut app);
    assert_eq!(
        app.queue.items[0].state,
        JobState::Cancelled { kept_steps: 2 }
    );
}

#[test]
fn a_failed_job_records_its_step_and_the_steps_it_kept() {
    let rerun = Arc::new(Mutex::new(Vec::new()));
    let mut app = app_with(
        "failed",
        vec![PathBuf::from("a.mp4")],
        failing(rerun.clone()),
    );
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.queue.items[0].rerun = vec![StepName::AsrWhisper];
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    assert!(
        app.queue.items[0].keep_settings,
        "a started job keeps its settings"
    );
    settle(&mut app);
    assert_eq!(
        rerun.lock().map(|seen| seen.clone()).unwrap_or_default(),
        [StepName::AsrWhisper]
    );
    assert!(
        app.queue.items[0].rerun.is_empty(),
        "once a step started, the pipeline holds the steps to run again"
    );
    assert_eq!(
        app.queue.items[0].state,
        JobState::Failed(Failure::new(
            Some(StepName::AsrWhisper),
            "step asr_whisper: out of memory".into(),
            vec![
                (StepName::ProbeDecode, FinishedStep::StillValid),
                (StepName::ShotScan, FinishedStep::StillValid),
            ],
        ))
    );
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, _) = render(&app);
    for expected in [
        "Failed at Hear the speech",
        "Listen with Whisper stopped with an error.",
        "step asr_whisper: out of memory",
        "The 2 finished steps are kept. Try Again continues after them. It starts at once.",
        "Try Again",
        "Show in Folder",
        "Show all 29 steps",
        "9 stages",
        "Translate on-screen text",
        "already done",
        "failed",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
}

#[test]
fn a_job_failing_before_it_starts_keeps_its_steps_to_run_again() {
    let mut app = app_with("locked", vec![PathBuf::from("a.mp4")], locked());
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.queue.items[0].rerun = vec![StepName::Adjudicate];
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    assert_eq!(
        app.queue.items[0].state,
        JobState::Failed(Failure::new(
            None,
            "lock the work directory: another process runs this job".into(),
            Vec::new(),
        ))
    );
    assert_eq!(app.queue.items[0].rerun, [StepName::Adjudicate]);
    let kept = crate::job_queue::services::queue_store::load(&app.env.queue_path).expect("kept");
    assert_eq!(
        kept.items[0].rerun,
        [StepName::Adjudicate],
        "and so does queue.json"
    );
}

#[test]
fn a_full_run_waits_while_its_videos_review_run_runs() {
    let mut app = app("guard", vec![PathBuf::from("a.mp4")]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    let review = queue_editing::queue_review(&mut app.queue, PathBuf::from("a.mp4"), 1);
    // A review lane holds the review run, with no thread behind it.
    if let Some(item) = app.queue.get_mut(review) {
        item.state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
    }
    assert!(app.review_lanes.hold(review, CancelToken::new()));
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    assert!(app.queue.items[0].state.is_waiting(), "the full run waits");
    assert!(app.queue.running, "and holds its lane");
    if let Some(item) = app.queue.get_mut(review) {
        item.state = JobState::FinishedBefore;
    }
    app.review_lanes.release(review);
    app.start_next();
    assert!(
        app.queue.items[0].state.is_running(),
        "it starts once the review run ends"
    );
    settle(&mut app);
    assert!(matches!(app.queue.items[0].state, JobState::Finished(_)));
}

/// A stand-in correction run that runs until `release` is set, or ends cancelled when its token
/// is.
fn held_until(release: Arc<AtomicBool>) -> RunJob {
    Arc::new(move |video, options, progress| {
        progress(Progress::StepStarted(StepName::Cues));
        while !release.load(Ordering::SeqCst) {
            if options.cancel.is_cancelled() {
                return Err(PipelineError::cancelled("step cues"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Ok(JobOutcome {
            work_dir: options.work_root.join("job"),
            subtitles: video.with_extension("srt"),
            report: options.work_root.join("job").join("report.md"),
            qc: QcReport::default(),
            ran: vec![StepName::Cues],
            skipped: Vec::new(),
        })
    })
}

#[test]
fn four_correction_runs_run_at_once_but_never_two_of_one_video() {
    let release = Arc::new(AtomicBool::new(false));
    let videos: Vec<PathBuf> = (1..=5)
        .map(|n| PathBuf::from(format!("/v/Dressrosa {n}.mp4")))
        .collect();
    let mut app = app_with("four-lanes", videos.clone(), held_until(release.clone()));
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    for item in &mut app.queue.items {
        item.state = JobState::FinishedBefore;
    }
    let reviews: Vec<JobId> = videos
        .iter()
        .map(|video| queue_editing::queue_review(&mut app.queue, video.clone(), 1))
        .collect();
    app.start_next();
    let running = |app: &TbdSubtitlesApp, id| {
        app.queue
            .get(id)
            .is_some_and(|item| item.state.is_running())
    };
    for &id in &reviews[..4] {
        assert!(running(&app, id), "run {id} runs");
    }
    assert!(
        app.queue
            .get(reviews[4])
            .is_some_and(|i| i.state.is_waiting())
    );
    assert!(!app.review_lanes.free(), "the fifth waits for a lane");
    // A second correction run of a video whose run runs waits, even with lanes free.
    let again = queue_editing::queue_review(&mut app.queue, videos[0].clone(), 1);
    assert_ne!(again, reviews[0]);
    app.apply(vec![
        Action::from(JobQueueEvent::Cancel(reviews[1])),
        Action::from(JobQueueEvent::Cancel(reviews[2])),
    ]);
    for _ in 0..500 {
        app.poll();
        let cancelled = |id| {
            app.queue
                .get(id)
                .is_some_and(|i| matches!(i.state, JobState::Cancelled { .. }))
        };
        if cancelled(reviews[1]) && cancelled(reviews[2]) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(running(&app, reviews[4]), "the fifth takes a freed lane");
    assert!(app.review_lanes.free(), "a lane is free");
    assert!(
        app.queue.get(again).is_some_and(|i| i.state.is_waiting()),
        "yet the second run of Dressrosa 1 waits"
    );
    release.store(true, Ordering::SeqCst);
    settle(&mut app);
    for id in [reviews[0], reviews[3], reviews[4], again] {
        assert!(
            app.queue
                .get(id)
                .is_some_and(|i| matches!(i.state, JobState::Finished(_))),
            "run {id} finished"
        );
    }
}

#[test]
fn a_saved_correction_queues_a_review_run_that_runs_at_once() {
    let root = std::env::temp_dir().join(format!("tbd-app-review-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let video = root.join("Dressrosa 13.mp4");
    std::fs::write(&video, b"video").expect("video");
    let mut app =
        TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), vec![video.clone()]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    let job = root.join("work").join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(&video).expect("c"),
    ));
    std::fs::create_dir_all(&job).expect("job");
    let sheet = r#"[{"id":"U1","start_s":10.0,"end_s":11.0,"words":[],"locked":[],"line":"U1","hypotheses":[["P",["blame!"]],["W",["flavor!"]]]}]"#;
    let adjudicated = r#"{"lines":[{"id":"U1","t":"Blaver!","f":["UNSURE"]}],
        "findings":{"missing_ids":[],"duplicate_ids":[],"unknown_ids":[],"novel":[],"removed_locked":[],"too_fast":[]},
        "calls":1,"input_tokens":0,"output_tokens":0,"cost_usd":0.0}"#;
    std::fs::write(job.join("sheet.json"), sheet).expect("sheet");
    std::fs::write(job.join("adjudicated.json"), adjudicated).expect("adjudicated");
    let qc = QcReport {
        findings: vec![job_model::report::QcFinding {
            check: job_model::report::QcCheck::Unsure,
            time_s: 10.0,
            text: "Blaver!".into(),
            detail: "U1".into(),
            utterance: Some("U1".into()),
        }],
        ..QcReport::default()
    };
    std::fs::write(job.join("qc.json"), serde_json::to_string(&qc).expect("qc")).expect("qc");
    let id = app.queue.items[0].id;
    app.apply(vec![
        Action::from(JobQueueEvent::Select(id)),
        Action::ShowTab(DetailTab::CheckLines),
    ]);
    assert_eq!(app.detail_tab(id), DetailTab::CheckLines);
    let (text, _) = render(&app);
    for expected in [
        "Overview",
        "Check Lines",
        "To Check",
        "Blaver!",
        "Parakeet",
        "flavor!",
        "Looks Right",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    app.apply(vec![
        Action::from(crate::line_review::events::ReviewEvent::Pick("P".into())),
        Action::from(crate::line_review::events::ReviewEvent::Save),
    ]);
    assert!(job.join("review.json").exists());
    let review_runs = app
        .queue
        .items
        .iter()
        .filter(|i| i.kind == crate::job_queue::models::queue::JobKind::Review)
        .count();
    assert_eq!(review_runs, 1);
    assert_eq!(app.queue.items[1].corrections, 1);
    let (text, _) = render(&app);
    assert!(
        text.contains("Updating subtitles · 1 correction"),
        "the correction run shows on its video's row: {text}"
    );
    let rows = text.lines().filter(|line| *line == "Dressrosa 13").count();
    assert_eq!(
        rows, 2,
        "one row for the video, and the header's title: {text}"
    );
    settle(&mut app);
    assert!(matches!(app.queue.items[1].state, JobState::Finished(_)));
    let (text, _) = render(&app);
    assert!(text.contains("Subtitles ready"), "{text}");
    app.apply(vec![Action::ShowTab(DetailTab::Overview)]);
    assert!(app.review.is_none(), "Overview closes the lines to check");
    assert_eq!(app.detail_tab(id), DetailTab::Overview);
    app.apply(vec![Action::ShowTab(DetailTab::CheckLines)]);
    assert_eq!(app.detail_tab(id), DetailTab::CheckLines);
    app.apply(vec![Action::from(JobQueueEvent::RunAgain(id))]);
    assert!(
        app.queue
            .get(id)
            .is_some_and(|item| item.state.is_running()),
        "run again at once"
    );
    assert!(app.review.is_none(), "a job run again leaves Check Lines");
    let (text, _) = render(&app);
    assert!(text.contains("Cancel"), "its running view shows: {text}");
    settle(&mut app);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_window_draws_in_the_desktops_scheme() {
    use crate::core::ui::palette::{DARK, LIGHT};
    let mut app = app("scheme", Vec::new());
    let context = egui::Context::default();
    theme::install(&context);
    for (scheme, palette) in [(Scheme::Dark, &DARK), (Scheme::Light, &LIGHT)] {
        app.scheme = scheme;
        theme::follow(&context, app.scheme);
        let mut drawn = None;
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            drawn = Some(ui.visuals().clone());
        });
        output.textures_delta.clear();
        let visuals = drawn.expect("a frame ran");
        assert_eq!(visuals.dark_mode, scheme == Scheme::Dark);
        assert_eq!(visuals.panel_fill, palette.window);
        assert_eq!(visuals.selection.bg_fill, palette.selection_fill());
    }
}

#[test]
fn a_change_of_the_desktops_scheme_is_followed() {
    let mut app = app("scheme-change", Vec::new());
    assert_eq!(
        app.scheme,
        Scheme::Light,
        "the tests start no portal thread"
    );
    let (send, schemes) = std::sync::mpsc::channel();
    app.pending.scheme = Some(schemes);
    send.send(Scheme::Dark).expect("send");
    app.poll();
    assert_eq!(app.scheme, Scheme::Dark);
    drop(send);
    app.poll();
    assert!(
        app.pending.scheme.is_none(),
        "a closed portal thread ends the wait"
    );
    assert_eq!(app.scheme, Scheme::Dark);
}

#[test]
fn an_open_the_desktop_never_answers_ends_in_a_red_toast() {
    use crate::application::background::{OPEN_DEADLINE, OpenRequest};
    use crate::core::toast::ToastKind;
    let mut app = app("open-deadline", Vec::new());
    let asked = Instant::now();
    let (send, answer) = std::sync::mpsc::channel();
    app.pending.opens.push(OpenRequest {
        failed: "Dressrosa 17.mp4 could not open".to_string(),
        asked,
        answer,
    });
    assert_eq!(
        app.pending.next_open_deadline(),
        Some(asked + OPEN_DEADLINE)
    );
    app.poll_opens(asked + OPEN_DEADLINE - Duration::from_secs(1));
    assert_eq!(app.pending.opens.len(), 1, "still within its time");
    assert!(app.toasts.shown().is_empty());
    app.poll_opens(asked + OPEN_DEADLINE);
    assert!(
        app.pending.opens.is_empty(),
        "a timed-out open is forgotten"
    );
    let toast = app.toasts.shown().last().expect("a toast");
    assert_eq!(toast.kind, ToastKind::Error);
    assert_eq!(
        toast.text,
        "Dressrosa 17.mp4 could not open: the desktop did not answer"
    );
    drop(send);
}

#[test]
fn asking_for_a_chooser_while_one_is_open_says_so() {
    use crate::application::background::Chooser;
    use crate::core::toast::ToastKind;
    let mut app = app("chooser-open", Vec::new());
    let (_send, answer) = std::sync::mpsc::channel();
    app.pending.chooser = Some((Chooser::QueueVideos, answer));
    app.choose_for_queue(true);
    let toast = app.toasts.shown().last().expect("a toast");
    assert_eq!(toast.kind, ToastKind::Info);
    assert_eq!(toast.text, "A file chooser is already open.");
    assert!(matches!(
        app.pending.chooser,
        Some((Chooser::QueueVideos, _))
    ));
}
