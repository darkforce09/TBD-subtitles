use std::sync::Mutex;
use std::time::{Duration, Instant};

use job_model::StepName;
use job_model::report::QcReport;
use pipeline::progress::Progress;
use pipeline::{JobOutcome, PipelineError};

use super::*;
use crate::core::color_scheme::Scheme;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::progress::JobProgress;
use crate::job_queue::models::queue::{Failure, JobState};
use crate::job_queue::services::job_runner::RunJob;
use crate::settings::events::SettingsEvent;

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
    let context = egui::Context::default();
    theme::install(&context);
    theme::follow(&context, app.scheme);
    let mut text = String::new();
    let mut actions = Vec::new();
    // Panels settle their sizes on the first frame.
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1800.0),
            )),
            ..Default::default()
        };
        let mut output = context.run_ui(input, |ui| actions = app.frame_ui(ui));
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

fn videos(app: &TbdSubtitlesApp) -> Vec<PathBuf> {
    app.queue.items.iter().map(|i| i.video.clone()).collect()
}

#[test]
fn an_empty_queue_says_how_to_add_videos() {
    let (text, actions) = render(&app("empty", Vec::new()));
    assert!(text.contains("Queue"), "{text}");
    assert!(text.contains("No videos queued"), "{text}");
    assert!(text.contains("Add videos"), "{text}");
    assert!(actions.is_empty());
}

#[test]
fn queued_videos_show_by_file_name_and_wait_for_start() {
    let app = app("names", vec![PathBuf::from("/videos/Dressrosa 08.mp4")]);
    let (text, _) = render(&app);
    assert!(text.contains("Dressrosa 08.mp4"), "{text}");
    assert!(text.contains("1 waiting"), "{text}");
    assert!(text.contains("Models are missing"), "{text}");
    assert!(!text.contains("No videos queued"), "{text}");
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
    let (text, _) = render(&app);
    assert!(text.contains("2 finished steps kept"), "{text}");
    app.apply(vec![
        Action::from(JobQueueEvent::Pause),
        Action::from(JobQueueEvent::Retry(id)),
    ]);
    assert!(app.queue.items[0].state.is_waiting());
    assert!(
        app.queue.items[0].keep_settings,
        "a retry keeps its own settings"
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
        JobState::Failed(Failure {
            step: Some(StepName::AsrWhisper),
            message: "step asr_whisper: out of memory".into(),
            kept_steps: 2,
        })
    );
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, _) = render(&app);
    for expected in [
        "Failed at Hear the speech.",
        "Listen with Whisper: step asr_whisper: out of memory",
        "2 finished steps kept",
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
        JobState::Failed(Failure {
            step: None,
            message: "lock the work directory: another process runs this job".into(),
            kept_steps: 0,
        })
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
    let review = queue_editing::queue_review(&mut app.queue, PathBuf::from("a.mp4"));
    // The review lane holds the review run, with no thread behind it.
    if let Some(item) = app.queue.get_mut(review) {
        item.state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
    }
    app.review_cancel = Some((review, CancelToken::new()));
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    assert!(app.queue.items[0].state.is_waiting(), "the full run waits");
    assert!(app.queue.running, "and holds its lane");
    if let Some(item) = app.queue.get_mut(review) {
        item.state = JobState::FinishedBefore;
    }
    app.review_cancel = None;
    app.start_next();
    assert!(
        app.queue.items[0].state.is_running(),
        "it starts once the review run ends"
    );
    settle(&mut app);
    assert!(matches!(app.queue.items[0].state, JobState::Finished(_)));
}

#[test]
fn the_settings_page_shows_the_form_models_and_checks() {
    let mut app = app("settings", Vec::new());
    app.apply(vec![Action::ShowPage(Page::Settings)]);
    let (text, _) = render(&app);
    for expected in [
        "Settings",
        "Models folder",
        "Subtitle format",
        "SRT (default)",
        "Models and runtime",
        "parakeet-tdt-0.6b-v2",
        "missing",
        "Download missing",
        "This machine",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
}

#[test]
fn an_edit_is_saved_only_by_save() {
    let mut app = app("save", Vec::new());
    let mut draft = app.settings.draft.clone();
    draft.cut_score = 33.0;
    let before = std::fs::read_to_string(&app.env.settings_path).expect("scratch settings");
    app.apply(vec![Action::from(SettingsEvent::Edit(draft))]);
    assert!(app.settings.has_edits());
    assert_eq!(
        std::fs::read_to_string(&app.env.settings_path).expect("unchanged"),
        before
    );
    app.apply(vec![Action::from(SettingsEvent::Save)]);
    assert!(!app.settings.has_edits());
    assert!(
        std::fs::read_to_string(&app.env.settings_path)
            .expect("saved")
            .contains("33")
    );
}

#[test]
fn a_finished_job_shows_its_report() {
    let root = std::env::temp_dir().join(format!("tbd-app-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let video = root.join("Dressrosa 12.mp4");
    std::fs::write(&video, b"video").expect("video");
    let mut app =
        TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), vec![video.clone()]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    // What the pipeline leaves in the job's work directory.
    let job = root.join("work").join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(&video).expect("c"),
    ));
    std::fs::create_dir_all(&job).expect("job");
    let record = job_model::job::JobRecord {
        video: video.to_string_lossy().into_owned(),
        video_size: 5,
        video_modified_s: 0,
        settings: job_model::job::JobSettings::with_glossary(vec![]),
        models_dir: None,
        corrections: None,
        steps: Default::default(),
    };
    let qc = QcReport {
        findings: vec![job_model::report::QcFinding {
            check: job_model::report::QcCheck::Unsure,
            time_s: 246.8,
            text: "Blaver!".into(),
            detail: "U0053".into(),
            utterance: Some("U0053".into()),
        }],
        ..QcReport::default()
    };
    std::fs::write(
        job.join("job.json"),
        serde_json::to_string(&record).expect("json"),
    )
    .expect("w");
    std::fs::write(
        job.join("qc.json"),
        serde_json::to_string(&qc).expect("json"),
    )
    .expect("w");
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(id))]);
    let (text, _) = render(&app);
    for expected in [
        "Passes the quality check",
        "Findings (1)",
        "Blaver!",
        "0:04:06.8",
        "Open in the video player",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    let _ = std::fs::remove_dir_all(&root);
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
    let id = app.queue.items[0].id;
    app.apply(vec![
        Action::from(JobQueueEvent::Select(id)),
        Action::from(crate::job_report::events::ReportEvent::Review(None)),
    ]);
    let (text, _) = render(&app);
    for expected in [
        "Review lines",
        "Blaver!",
        "Parakeet",
        "flavor!",
        "Save and time again",
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
    settle(&mut app);
    assert!(matches!(app.queue.items[1].state, JobState::Finished(_)));
    let (text, _) = render(&app);
    assert!(text.contains("Dressrosa 13.mp4 · 1 correction"), "{text}");
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
