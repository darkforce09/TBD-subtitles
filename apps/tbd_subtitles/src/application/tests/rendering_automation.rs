//! Automation: later starts' hand-offs, the watch folders' videos, the queue started for them
//! unless the owner paused it, the desktop told when a job ends while the window is away, and
//! what `logic` does without `ui`.

use std::path::Path;
use std::sync::mpsc::{Sender, channel};

use eframe::App as _;
use eframe::egui::{RawInput, UserAttentionType, ViewportCommand, ViewportId};

use super::*;
use crate::application::environment::NOTIFIED;
use crate::core::service_menu::Installed;
use crate::core::single_instance::HandOff;
use crate::job_queue::models::queue::JobKind;
use crate::job_queue::services::queued_history;
use crate::settings::models::page::RightClickEntry;

/// An application whose models are all on disk, with a channel for hand-offs into it.
fn ready(name: &str) -> (TbdSubtitlesApp, Sender<HandOff>) {
    let mut app = app(name, Vec::new());
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    let (send, receive) = channel();
    let wake = Arc::new(OnceLock::new());
    app.automate(false, false, Some((receive, wake.clone())));
    assert!(wake.get().is_some(), "the hand-offs wake the window");
    (app, send)
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

/// Run `logic` alone, as eframe does while the window is minimized when `minimized`, and return
/// the commands it sent the window.
fn logic(app: &mut TbdSubtitlesApp, minimized: bool) -> Vec<ViewportCommand> {
    let context = egui::Context::default();
    let mut input = RawInput::default();
    if let Some(root) = input.viewports.get_mut(&ViewportId::ROOT) {
        root.minimized = Some(minimized);
    }
    let mut frame = eframe::Frame::_new_kittest();
    let output = context.run_logic(&input, |ctx| app.logic(ctx, &mut frame));
    output.viewport_commands.into_values().flatten().collect()
}

/// A scratch folder for `name`'s videos, emptied.
fn folder(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("tbd-watch-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// A video file at `path`.
fn touch(path: &Path) -> PathBuf {
    std::fs::write(path, b"video").unwrap();
    path.to_path_buf()
}

#[test]
fn a_hand_off_that_starts_queues_starts_and_remembers_its_videos() {
    let (mut app, send) = ready("hand-off-start");
    let video = PathBuf::from("/videos/ep01.mkv");
    send.send(HandOff {
        videos: vec![video.clone()],
        start: true,
        raise: false,
    })
    .unwrap();
    app.poll();
    assert_eq!(videos(&app), vec![video.clone()]);
    assert!(
        app.queue.items[0].state.is_running(),
        "the queue started for it"
    );
    assert!(
        !app.raise,
        "a start that queues leaves the window where it is"
    );
    assert!(app.history.contains(&video));
    let kept = queued_history::load(&app.env.history_path);
    assert!(kept.contains(&video), "the history is written at once");
    settle(&mut app);
    assert!(matches!(app.queue.items[0].state, JobState::Finished(_)));
}

#[test]
fn a_hand_off_that_raises_brings_the_window_forward_through_logic() {
    let (mut app, send) = ready("hand-off-raise");
    send.send(HandOff {
        videos: vec![PathBuf::from("/videos/ep02.mkv")],
        start: false,
        raise: true,
    })
    .unwrap();
    let commands = logic(&mut app, true);
    assert_eq!(app.queue.items.len(), 1, "logic alone takes the hand-off");
    assert!(app.queue.items[0].state.is_waiting(), "it does not start");
    for expected in [
        ViewportCommand::Minimized(false),
        ViewportCommand::Focus,
        ViewportCommand::RequestUserAttention(UserAttentionType::Informational),
    ] {
        assert!(
            commands.contains(&expected),
            "{expected:?} not in {commands:?}"
        );
    }
    assert!(!app.raise && !app.attention);
    assert!(
        logic(&mut app, false).is_empty(),
        "the window is raised once"
    );
}

#[test]
fn a_minimized_launch_minimizes_the_window_once() {
    let mut app = app("launch-minimized", Vec::new());
    app.automate(false, true, None);
    let first = logic(&mut app, false);
    assert!(
        first.contains(&ViewportCommand::Minimized(true)),
        "{first:?}"
    );
    assert!(!logic(&mut app, true).contains(&ViewportCommand::Minimized(true)));
    assert!(app.presence.away, "a minimized window is away");
}

#[test]
fn a_launch_that_starts_runs_the_videos_it_was_given() {
    let mut app = app("launch-start", vec![PathBuf::from("/videos/ep03.mkv")]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.automate(true, true, None);
    assert!(app.queue.items[0].state.is_running());
    settle(&mut app);
}

#[test]
fn logic_alone_folds_in_the_runners_while_the_window_is_minimized() {
    let (mut app, send) = ready("logic-alone");
    send.send(HandOff {
        videos: vec![PathBuf::from("/videos/ep04.mkv")],
        start: true,
        raise: false,
    })
    .unwrap();
    for _ in 0..500 {
        logic(&mut app, true);
        if matches!(app.queue.items[0].state, JobState::Finished(_)) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("the job never finished without a frame drawn");
}

#[test]
fn a_watch_folder_queues_only_videos_never_queued_without_subtitles() {
    let (mut app, _send) = ready("watch-filter");
    app.apply(vec![Action::Queue(JobQueueEvent::Pause)]);
    let dir = folder("filter");
    let before = touch(&dir.join("before.mkv"));
    let subtitled = touch(&dir.join("subtitled.mkv"));
    touch(&dir.join("subtitled.srt"));
    let waiting = touch(&dir.join("waiting.mkv"));
    let fresh = touch(&dir.join("fresh.mkv"));
    app.history.record([before.clone()]);
    queue_editing::push(&mut app.queue, waiting.clone(), JobKind::Full);
    app.queue_found(vec![before, subtitled, waiting.clone(), fresh.clone()]);
    assert_eq!(videos(&app), vec![waiting, fresh.clone()]);
    assert!(app.history.contains(&fresh));
    assert!(
        app.toasts
            .shown()
            .iter()
            .any(|toast| toast.text == "Added 1 video from the watch folders.")
    );
    // A second scan finds it again; it is not queued twice.
    app.queue_found(vec![fresh]);
    assert_eq!(app.queue.items.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_watcher_follows_the_saved_watch_folders() {
    let (mut app, _send) = ready("watch-follow");
    assert!(app.watcher.folders().is_empty());
    let dir = folder("follow");
    app.settings.saved.watch_folders = vec![dir.clone()];
    app.poll();
    assert_eq!(app.watcher.folders(), [dir.clone()].as_slice());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn automation_never_starts_a_queue_the_owner_paused_until_start() {
    let (mut app, send) = ready("paused");
    app.apply(vec![Action::Queue(JobQueueEvent::Pause)]);
    assert!(app.paused_by_owner);
    let dir = folder("paused");
    let found = touch(&dir.join("found.mkv"));
    app.queue_found(vec![found]);
    send.send(HandOff {
        videos: vec![PathBuf::from("/videos/ep05.mkv")],
        start: true,
        raise: false,
    })
    .unwrap();
    app.poll();
    assert_eq!(app.queue.items.len(), 2);
    assert!(!app.queue.running);
    assert!(app.queue.items.iter().all(|item| item.state.is_waiting()));
    app.apply(vec![Action::Queue(JobQueueEvent::Start)]);
    assert!(!app.paused_by_owner, "Start ends the owner's pause");
    settle(&mut app);
    send.send(HandOff {
        videos: vec![PathBuf::from("/videos/ep06.mkv")],
        start: true,
        raise: false,
    })
    .unwrap();
    app.poll();
    assert!(
        app.queue.running_job().is_some(),
        "automation starts it again"
    );
    settle(&mut app);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn videos_queued_while_a_model_is_missing_tell_the_desktop_and_start_once_it_is_on_disk() {
    let mut app = app("models-missing", Vec::new());
    assert!(app.models_missing());
    app.presence.away = true;
    let (send, receive) = channel();
    app.automate(false, false, Some((receive, Arc::new(OnceLock::new()))));
    send.send(HandOff {
        videos: vec![PathBuf::from("/videos/ep07.mkv")],
        start: true,
        raise: false,
    })
    .unwrap();
    app.poll();
    assert_eq!(
        notified(&app),
        vec!["1 video queued: Download the missing models in Settings to start them."]
    );
    assert!(app.queue.running && app.queue.items[0].state.is_waiting());
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.poll();
    assert!(
        app.queue.items[0].state.is_running(),
        "the models are on disk"
    );
    settle(&mut app);
}

#[test]
fn a_job_that_ends_while_the_window_is_away_tells_the_desktop() {
    let (mut app, _send) = ready("notify-away");
    app.presence.away = true;
    app.apply(vec![
        Action::QueueVideos(vec![PathBuf::from("/videos/ep08.mkv")]),
        Action::Queue(JobQueueEvent::Start),
    ]);
    settle(&mut app);
    let told = notified(&app);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].starts_with("Subtitles ready: "), "{told:?}");
    assert!(told[0].ends_with("The quality check passed."), "{told:?}");
    assert!(app.attention, "the taskbar entry asks for attention");
}

#[test]
fn a_job_that_ends_with_the_window_in_front_leaves_the_desktop_alone() {
    let (mut app, _send) = ready("notify-front");
    app.apply(vec![
        Action::QueueVideos(vec![PathBuf::from("/videos/ep09.mkv")]),
        Action::Queue(JobQueueEvent::Start),
    ]);
    settle(&mut app);
    assert!(notified(&app).is_empty());
    assert!(!app.attention);
}

#[test]
fn a_failed_job_tells_the_desktop_where_it_failed() {
    let mut app = app_with(
        "notify-failed",
        Vec::new(),
        failing(Arc::new(Mutex::new(Vec::new()))),
    );
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.presence.away = true;
    app.apply(vec![
        Action::QueueVideos(vec![PathBuf::from("/videos/ep10.mkv")]),
        Action::Queue(JobQueueEvent::Start),
    ]);
    settle(&mut app);
    let told = notified(&app);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].contains("failed: At "), "{told:?}");
}

#[test]
fn every_video_queued_is_remembered_across_windows() {
    let root = std::env::temp_dir().join(format!("tbd-app-history-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let video = PathBuf::from("/videos/ep11.mkv");
    let first = TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), Vec::new());
    let mut first = first;
    first.apply(vec![Action::QueueVideos(vec![video.clone()])]);
    let history_path = first.env.history_path.clone();
    assert!(queued_history::load(&history_path).contains(&video));
    drop(first);
    // A history lost meanwhile is seeded again from the kept queue.
    std::fs::remove_file(&history_path).unwrap();
    let second = TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), Vec::new());
    assert!(second.history.contains(&video));
    assert!(queued_history::load(&history_path).contains(&video));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_right_click_entry_says_how_writing_the_service_menu_went() {
    let menu = PathBuf::from("/menus/tbd-subtitles.desktop");
    assert_eq!(
        actions::right_click_entry(None),
        RightClickEntry::NotInstalled
    );
    assert_eq!(
        actions::right_click_entry(Some(Ok(Installed::Written(menu.clone())))),
        RightClickEntry::Installed(menu.clone())
    );
    assert_eq!(
        actions::right_click_entry(Some(Ok(Installed::Unchanged(menu.clone())))),
        RightClickEntry::Installed(menu)
    );
    let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "read-only");
    assert_eq!(
        actions::right_click_entry(Some(Err(denied))),
        RightClickEntry::Failed("read-only".into())
    );
}
