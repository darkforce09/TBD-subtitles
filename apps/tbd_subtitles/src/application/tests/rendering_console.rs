//! The log window: opened from the toolbar and Ctrl+L, its lines, levels, search and Clear,
//! rendered headless.

use tracing::Level;

use super::*;
use crate::log_console::events::LogConsoleEvent;

/// A key pressed with Ctrl.
fn ctrl(key: egui::Key) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: egui::Modifiers::COMMAND,
    }
}

/// The actions of a primary click at `at`, given to a frame after one that lays the window out:
/// egui finds what is under the pointer from the frame before.
fn click(app: &TbdSubtitlesApp, at: egui::Pos2) -> Vec<Action> {
    let context = egui::Context::default();
    theme::install(&context);
    let frame = |events| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1800.0),
            )),
            events,
            ..Default::default()
        };
        let mut actions = Vec::new();
        let mut output = context.run_ui(input, |ui| actions.extend(app.frame_ui(ui)));
        output.textures_delta.clear();
        actions
    };
    frame(vec![egui::Event::PointerMoved(at)]);
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(vec![button(true), button(false)])
}

/// An application with the log window open and three lines logged.
fn open_with_lines(name: &str) -> TbdSubtitlesApp {
    let mut app = app(name, Vec::new());
    app.env
        .log
        .push(Level::DEBUG, "child_process", "ffmpeg[42] frame=12".into());
    app.env
        .log
        .push(Level::INFO, "job", "Dressrosa 12: cues: started".into());
    app.env.log.push(
        Level::ERROR,
        "job",
        "Dressrosa 12: stopped: out of memory".into(),
    );
    app.apply(vec![Action::ShowLog(true)]);
    app
}

#[test]
fn the_log_window_opens_from_ctrl_l_and_the_toolbar() {
    let app = app("log-open", Vec::new());
    let (text, actions) = render(&app);
    assert!(!text.contains("Open Log File"), "closed at first: {text}");
    assert!(actions.is_empty());
    let (_, actions) = render_with(&app, vec![ctrl(egui::Key::L)]);
    assert_eq!(actions, [Action::ShowLog(true)]);
    // The log button sits left of the gear at the toolbar's right end.
    assert_eq!(
        click(&app, egui::pos2(1338.0, 26.0)),
        [Action::ShowLog(true)]
    );
}

#[test]
fn the_open_log_window_shows_every_line_with_its_level_and_source() {
    let mut app = open_with_lines("log-lines");
    assert!(app.log_window);
    assert_eq!(
        app.console.len(),
        3,
        "opening reads the lines logged so far"
    );
    let (text, actions) = render(&app);
    assert!(
        actions.is_empty(),
        "an idle frame asks for nothing: {actions:?}"
    );
    for expected in [
        "Errors",
        "Warnings",
        "Info",
        "Debug",
        "Copy",
        "Clear",
        "Open Log File",
        "ffmpeg[42] frame=12",
        "Dressrosa 12: cues: started",
        "Dressrosa 12: stopped: out of memory",
        "ERROR  job",
        "DEBUG  child_process",
        "3 lines",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    app.env
        .log
        .push(Level::WARN, "child_process", "claude[7] exited 1".into());
    app.poll();
    let (text, _) = render(&app);
    assert!(
        text.contains("claude[7] exited 1"),
        "new lines arrive: {text}"
    );
    assert!(text.contains("4 lines"), "{text}");
}

#[test]
fn levels_and_the_search_narrow_the_lines_and_clear_empties_them() {
    let mut app = open_with_lines("log-filter");
    app.apply(vec![Action::LogConsole(LogConsoleEvent::Level(
        Level::INFO,
    ))]);
    let (text, _) = render(&app);
    assert!(!text.contains("frame=12"), "debug lines hide: {text}");
    assert!(text.contains("2 of 3 lines"), "{text}");
    app.apply(vec![Action::LogConsole(LogConsoleEvent::Search(
        "MEMORY".into(),
    ))]);
    let (text, _) = render(&app);
    assert!(!text.contains("cues: started"), "{text}");
    assert!(text.contains("stopped: out of memory"), "{text}");
    app.apply(vec![Action::LogConsole(LogConsoleEvent::Search(
        "nothing like it".into(),
    ))]);
    let (text, _) = render(&app);
    assert!(text.contains("No line matches the filter."), "{text}");
    app.apply(vec![Action::LogConsole(LogConsoleEvent::Clear)]);
    assert!(app.env.log.since(0).is_empty(), "the buffer is cleared too");
    let (text, _) = render(&app);
    assert!(text.contains("Nothing is logged yet."), "{text}");
}

#[test]
fn lines_logged_while_the_window_is_closed_show_when_it_opens_again() {
    let mut app = open_with_lines("log-reopen");
    app.apply(vec![Action::ShowLog(false)]);
    let (text, _) = render(&app);
    assert!(!text.contains("Open Log File"), "closed: {text}");
    app.env
        .log
        .push(Level::INFO, "pipeline::runner", "while closed".into());
    app.poll();
    assert_eq!(app.console.len(), 3, "nothing is read while closed");
    app.apply(vec![Action::ShowLog(true)]);
    assert_eq!(app.console.len(), 4);
    let (text, _) = render(&app);
    assert!(text.contains("while closed"), "{text}");
}

#[test]
fn an_action_is_logged_in_a_few_words() {
    let long = Action::QueueVideos(vec![PathBuf::from("x".repeat(400))]);
    let described = long.describe();
    assert_eq!(described.chars().count(), 161);
    assert!(described.ends_with('…'));
    assert_eq!(Action::ShowLog(true).describe(), "ShowLog(true)");
}
