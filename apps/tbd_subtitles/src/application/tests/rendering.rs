use super::*;
use crate::settings::events::SettingsEvent;

/// An application over files in a scratch folder of its own, never the owner's home.
fn app(name: &str, videos: Vec<PathBuf>) -> TbdSubtitlesApp {
    let root = std::env::temp_dir().join(format!("tbd-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    TbdSubtitlesApp::new(Environment::scratch(&root), videos)
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

#[test]
fn an_empty_queue_says_how_to_add_videos() {
    let (text, actions) = render(&app("empty", Vec::new()));
    assert!(text.contains("Queue"), "{text}");
    assert!(text.contains("No videos queued"), "{text}");
    assert!(text.contains("Add videos"), "{text}");
    assert!(actions.is_empty());
}

#[test]
fn queued_videos_show_by_file_name() {
    let app = app("names", vec![PathBuf::from("/videos/Dressrosa 08.mp4")]);
    let (text, _) = render(&app);
    assert!(text.contains("Dressrosa 08.mp4"), "{text}");
    assert!(!text.contains("No videos queued"), "{text}");
}

#[test]
fn actions_change_the_queue_only_when_applied() {
    let mut app = app(
        "apply",
        vec![PathBuf::from("a.mp4"), PathBuf::from("b.mp4")],
    );
    app.apply(vec![
        Action::from(crate::job_queue::events::JobQueueEvent::Remove(0)),
        Action::QueueVideos(vec![PathBuf::from("b.mp4"), PathBuf::from("c.mp4")]),
    ]);
    assert_eq!(app.queue, [PathBuf::from("b.mp4"), PathBuf::from("c.mp4")]);
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
    app.apply(vec![Action::from(SettingsEvent::Edit(draft))]);
    assert!(app.settings.has_edits());
    assert!(!app.env.settings_path.exists());
    app.apply(vec![Action::from(SettingsEvent::Save)]);
    assert!(!app.settings.has_edits());
    assert!(app.env.settings_path.exists());
}
