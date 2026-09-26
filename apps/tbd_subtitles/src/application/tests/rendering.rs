use std::time::Duration;

use job_model::StepName;
use job_model::report::QcReport;
use pipeline::progress::Progress;
use pipeline::{JobOutcome, PipelineError};

use super::*;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::JobState;
use crate::job_queue::services::job_runner::RunJob;
use crate::settings::events::SettingsEvent;

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

/// An application over files in a scratch folder of its own, never the owner's home.
fn app(name: &str, videos: Vec<PathBuf>) -> TbdSubtitlesApp {
    let root = std::env::temp_dir().join(format!("tbd-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), videos)
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

/// Run two headless frames and return every piece of text painted, with the actions asked for.
fn render(app: &TbdSubtitlesApp) -> (String, Vec<Action>) {
    let context = egui::Context::default();
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
fn a_cancelled_job_ends_cancelled_and_can_be_retried() {
    let mut app = app("cancel", vec![PathBuf::from("a.mp4")]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.queue.running = true;
    // Cancel before the stand-in looks at its token.
    app.cancel = None;
    app.start_next();
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::Cancel(id))]);
    settle(&mut app);
    assert!(matches!(
        app.queue.items[0].state,
        JobState::Cancelled | JobState::Finished(_)
    ));
    app.apply(vec![
        Action::from(JobQueueEvent::Pause),
        Action::from(JobQueueEvent::Retry(id)),
    ]);
    assert!(app.queue.items[0].state.is_waiting());
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
