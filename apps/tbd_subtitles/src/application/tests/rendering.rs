use super::*;

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
                egui::vec2(1100.0, 700.0),
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
    let (text, actions) = render(&TbdSubtitlesApp::new(Vec::new()));
    assert!(text.contains("Queue"), "{text}");
    assert!(text.contains("No videos queued"), "{text}");
    assert!(actions.is_empty());
}

#[test]
fn queued_videos_show_by_file_name() {
    let app = TbdSubtitlesApp::new(vec![PathBuf::from("/videos/Dressrosa 08.mp4")]);
    let (text, _) = render(&app);
    assert!(text.contains("Dressrosa 08.mp4"), "{text}");
    assert!(!text.contains("No videos queued"), "{text}");
}

#[test]
fn actions_change_the_queue_only_when_applied() {
    let mut app = TbdSubtitlesApp::new(vec![PathBuf::from("a.mp4"), PathBuf::from("b.mp4")]);
    app.apply(vec![
        Action::from(crate::job_queue::events::JobQueueEvent::Remove(0)),
        Action::QueueVideos(vec![PathBuf::from("b.mp4"), PathBuf::from("c.mp4")]),
    ]);
    assert_eq!(app.queue, [PathBuf::from("b.mp4"), PathBuf::from("c.mp4")]);
}
