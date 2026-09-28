//! The toolbar, the sidebar, the toasts and the shortcuts, rendered headless.

use super::*;
use crate::job_queue::models::queue::JobKind;

/// A key pressed with `modifiers`.
fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// The queue actions among `actions`.
fn queue_events(actions: &[Action]) -> Vec<JobQueueEvent> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Queue(event) => Some(event.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_running_queue_shows_its_sections_and_pause_after_this_video() {
    let mut app = app(
        "sections",
        ["/v/a.mp4", "/v/b.mp4", "/v/c.mp4"]
            .map(PathBuf::from)
            .to_vec(),
    );
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    let (text, _) = render(&app);
    assert!(text.contains("Start Queue"), "{text}");
    app.queue.items[0].state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
    app.queue.items[2].state = JobState::FinishedBefore;
    app.queue.running = true;
    // The full lane holds the running job, with no thread behind it.
    app.cancel = Some((app.queue.items[0].id, CancelToken::new()));
    let (text, actions) = render(&app);
    for expected in [
        "Pause After This Video",
        "NOW",
        "UP NEXT",
        "DONE",
        "Starting…",
        "Waiting · next in line",
        "Subtitles ready",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(!text.contains("Start Queue"), "{text}");
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    app.apply(vec![Action::from(JobQueueEvent::Pause)]);
    let (text, _) = render(&app);
    for expected in [
        "Resume Queue",
        "Pauses after a",
        "The queue pauses after a finishes.",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
}

#[test]
fn a_waiting_correction_run_does_not_enable_start() {
    let mut app = app("correction-start", vec![PathBuf::from("/v/a.mp4")]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.queue.items[0].state = JobState::FinishedBefore;
    queue_editing::queue_review(&mut app.queue, PathBuf::from("/v/a.mp4"), 1);
    let (text, _) = render(&app);
    assert!(text.contains("Nothing is waiting"), "{text}");
    assert!(text.contains("Updating subtitles · 1 correction"), "{text}");
    assert_eq!(
        app.queue
            .items
            .iter()
            .filter(|i| i.kind == JobKind::Review)
            .count(),
        1
    );
}

#[test]
fn delete_removes_the_selected_row_and_undo_puts_it_back() {
    let mut app = app("undo", ["/v/a.mp4", "/v/b.mp4"].map(PathBuf::from).to_vec());
    let b = app.queue.items[1].id;
    app.apply(vec![Action::from(JobQueueEvent::Select(b))]);
    let (_, actions) = render_with(&app, vec![key(egui::Key::Delete, egui::Modifiers::NONE)]);
    assert_eq!(queue_events(&actions), [JobQueueEvent::Remove(b)]);
    app.apply(actions);
    assert_eq!(videos(&app), [PathBuf::from("/v/a.mp4")]);
    assert_eq!(app.queue.selected, Some(0), "the row before it is selected");
    let (text, _) = render(&app);
    assert!(
        text.contains("Removed b. Its files stay on disk."),
        "{text}"
    );
    assert!(text.contains("Undo"), "{text}");
    let toast = app.toasts.shown()[0].id;
    app.apply(vec![Action::ToastButton(toast)]);
    assert_eq!(
        videos(&app),
        [PathBuf::from("/v/a.mp4"), PathBuf::from("/v/b.mp4")]
    );
    assert_eq!(app.queue.selected, Some(b), "the row comes back selected");
    assert!(
        app.toasts.shown().is_empty(),
        "the toast went with its button"
    );
    let (text, _) = render(&app);
    assert_eq!(
        text.lines().filter(|line| *line == "b").count(),
        2,
        "one row, and the selected job's title: {text}"
    );
}

#[test]
fn only_the_row_removed_last_comes_back() {
    let mut app = app(
        "undo-last",
        ["/v/a.mp4", "/v/b.mp4", "/v/c.mp4"]
            .map(PathBuf::from)
            .to_vec(),
    );
    app.apply(vec![
        Action::from(JobQueueEvent::Remove(0)),
        Action::from(JobQueueEvent::Remove(1)),
    ]);
    let undo: Vec<&Action> = app
        .toasts
        .shown()
        .iter()
        .filter_map(|toast| toast.action.as_ref().map(|(_, action)| action))
        .collect();
    assert_eq!(
        undo,
        [&Action::Queue(JobQueueEvent::Undo(1))],
        "the older Undo went with its toast"
    );
    app.apply(vec![Action::from(JobQueueEvent::Undo(0))]);
    assert_eq!(
        videos(&app),
        [PathBuf::from("/v/c.mp4")],
        "a stale Undo does nothing"
    );
    let toast = app.toasts.shown()[0].id;
    app.apply(vec![Action::ToastButton(toast)]);
    assert_eq!(
        videos(&app),
        [PathBuf::from("/v/b.mp4"), PathBuf::from("/v/c.mp4")]
    );
}

#[test]
fn a_video_already_in_the_list_is_not_put_back() {
    let mut app = app("already", vec![PathBuf::from("/v/a.mp4")]);
    app.apply(vec![Action::from(JobQueueEvent::Remove(0))]);
    app.apply(vec![Action::QueueVideos(vec![PathBuf::from("/v/a.mp4")])]);
    let toast = app.toasts.shown()[0].id;
    app.apply(vec![Action::ToastButton(toast)]);
    assert_eq!(videos(&app), [PathBuf::from("/v/a.mp4")], "not twice");
    let (text, _) = render(&app);
    assert!(text.contains("a is already in the list."), "{text}");
}

#[test]
fn a_job_tried_again_while_the_full_lane_runs_waits_for_start_queue() {
    let mut app = app(
        "first-in-line",
        ["/v/a.mp4", "/v/b.mp4"].map(PathBuf::from).to_vec(),
    );
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.queue.items[0].state = JobState::Running(Box::new(JobProgress::new(Instant::now())));
    app.cancel = Some((0, CancelToken::new()));
    app.queue.items[1].state = JobState::Cancelled { kept_steps: 0 };
    app.apply(vec![Action::from(JobQueueEvent::TryAgain(1, None))]);
    assert!(app.queue.items[1].state.is_waiting());
    let (text, _) = render(&app);
    assert!(
        text.contains("b is first in line. Press Start Queue to run it."),
        "{text}"
    );
}

#[test]
fn keys_add_videos_or_a_folder_open_settings_and_move_through_the_rows() {
    let mut app = app("keys", ["/v/a.mp4", "/v/b.mp4"].map(PathBuf::from).to_vec());
    let ctrl = egui::Modifiers::CTRL | egui::Modifiers::COMMAND;
    let (_, actions) = render_with(&app, vec![key(egui::Key::O, ctrl)]);
    assert_eq!(actions, [Action::Queue(JobQueueEvent::AddVideos)]);
    let (_, actions) = render_with(&app, vec![key(egui::Key::O, ctrl | egui::Modifiers::SHIFT)]);
    assert_eq!(actions, [Action::Queue(JobQueueEvent::AddFolder)]);
    let (_, actions) = render_with(&app, vec![key(egui::Key::Comma, ctrl)]);
    assert_eq!(actions, [Action::ShowSettings(true)]);
    let down = key(egui::Key::ArrowDown, egui::Modifiers::NONE);
    let (_, actions) = render_with(&app, vec![down.clone()]);
    assert_eq!(queue_events(&actions), [JobQueueEvent::Select(0)]);
    app.apply(actions);
    let (_, actions) = render_with(&app, vec![down.clone()]);
    assert_eq!(queue_events(&actions), [JobQueueEvent::Select(1)]);
    app.apply(actions);
    let (_, actions) = render_with(&app, vec![down]);
    assert!(actions.is_empty(), "the last row stays selected");
}

#[test]
fn run_again_with_the_settings_it_ran_with_runs_nothing() {
    let root = std::env::temp_dir().join(format!("tbd-app-run-again-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let video = root.join("Dressrosa 14.mp4");
    std::fs::write(&video, b"video").expect("video");
    let mut app =
        TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), vec![video.clone()]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.queue.items[0].state = JobState::FinishedBefore;
    let job = root.join("work").join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(&video).expect("c"),
    ));
    std::fs::create_dir_all(&job).expect("job");
    let settings = crate::settings::services::job_settings::job_settings(&app.settings.saved)
        .expect("the saved settings");
    let record = job_model::job::JobRecord {
        video: video.to_string_lossy().into_owned(),
        video_size: 5,
        video_modified_s: 0,
        settings,
        models_dir: None,
        corrections: None,
        steps: Default::default(),
    };
    std::fs::write(
        job.join("job.json"),
        serde_json::to_string(&record).expect("json"),
    )
    .expect("w");
    let id = app.queue.items[0].id;
    app.apply(vec![Action::from(JobQueueEvent::RunAgain(id))]);
    assert_eq!(app.queue.items[0].state, JobState::FinishedBefore);
    let (text, _) = render(&app);
    assert!(
        text.contains("Nothing to run again: Dressrosa 14 was made with the current settings."),
        "{text}"
    );
    std::fs::remove_file(job.join("job.json")).expect("removed");
    app.apply(vec![Action::from(JobQueueEvent::RunAgain(id))]);
    assert!(
        app.queue.items[0].state.is_running(),
        "a job with other settings runs again at once"
    );
    assert!(!app.queue.running);
    settle(&mut app);
    let _ = std::fs::remove_dir_all(&root);
}
