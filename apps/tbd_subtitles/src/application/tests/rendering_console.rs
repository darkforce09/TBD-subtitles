//! The log window: opened from the toolbar and Ctrl+L; its grouped lines with their writers,
//! filters, detail panel and Clear; and its Model Calls view, rendered headless.

use tracing::Level;

use super::*;
use job_model::model_call::ModelExchange;

use crate::core::log_buffer::Fresh;
use crate::log_console::events::LogConsoleEvent;
use crate::log_console::models::console::ConsoleView;
use crate::log_console::models::who::Who;
use crate::log_console::services::console_text;

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

fn log(app: &mut TbdSubtitlesApp, event: LogConsoleEvent) {
    app.apply(vec![Action::LogConsole(event)]);
}

/// An application with the log window open, four lines about Dressrosa 12 logged, and the model
/// call one of them sums up.
fn open_with_lines(name: &str) -> TbdSubtitlesApp {
    let mut app = app(name, Vec::new());
    let log = app.env.log.clone();
    log.push(
        Fresh::new(Level::DEBUG, "child_process", "ffmpeg[42] frame=12")
            .about("Dressrosa 12", Some("shot_scan")),
    );
    log.push(Fresh::new(Level::INFO, "job", "Step started").about("Dressrosa 12", Some("cues")));
    let mut summary = Fresh::new(
        Level::INFO,
        "inference::llm::call_log",
        "claude sonnet · words, batch 1 of 2: 12 lines answered in 4.2 s",
    )
    .about("Dressrosa 12", Some("adjudicate"));
    summary.call = Some("7-3".into());
    log.push(summary);
    log.push(
        Fresh::new(Level::ERROR, "job", "Job stopped: out of memory").about("Dressrosa 12", None),
    );
    log.push_call(
        ModelExchange {
            id: "7-3".into(),
            model: "sonnet".into(),
            purpose: "words, batch 1 of 2".into(),
            system: "RULES TEXT".into(),
            message: "U0012 hello there".into(),
            schema: "{}".into(),
            answer: "{\"lines\": []}".into(),
            seconds: 4.2,
            ..ModelExchange::default()
        },
        Some("Dressrosa 12".into()),
        Some("adjudicate".into()),
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
fn lines_show_under_a_header_per_step_with_a_chip_for_who_wrote_them() {
    let mut app = open_with_lines("log-lines");
    assert_eq!(
        app.console.activity.len(),
        4,
        "opening reads the lines so far"
    );
    assert_eq!(app.console.calls.len(), 1, "and the model calls");
    let (text, actions) = render(&app);
    assert!(
        actions.is_empty(),
        "an idle frame asks for nothing: {actions:?}"
    );
    let cues = console_text::header(Some("Dressrosa 12"), Some("cues"));
    for expected in [
        "Activity",
        "Model Calls",
        "Everyone",
        "Jobs",
        "Programs",
        "Program",
        "Job",
        "AI",
        "Open Log File",
        cues.as_str(),
        "ffmpeg[42] frame=12",
        "Step started",
        "Job stopped: out of memory",
        "4 lines",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    app.env.log.push(Fresh::new(
        Level::WARN,
        "child_process",
        "claude[7] exited 1",
    ));
    app.poll();
    let (text, _) = render(&app);
    assert!(
        text.contains("claude[7] exited 1"),
        "new lines arrive: {text}"
    );
    assert!(text.contains("5 lines"), "{text}");
}

#[test]
fn levels_writers_and_the_search_narrow_the_lines_and_clear_empties_them() {
    let mut app = open_with_lines("log-filter");
    log(&mut app, LogConsoleEvent::Level(Level::INFO));
    let (text, _) = render(&app);
    assert!(!text.contains("frame=12"), "debug lines hide: {text}");
    assert!(text.contains("3 of 4 lines"), "{text}");
    log(&mut app, LogConsoleEvent::Who(Some(Who::Ai)));
    let (text, _) = render(&app);
    assert!(text.contains("words, batch 1 of 2"), "{text}");
    assert!(!text.contains("Step started"), "{text}");
    log(&mut app, LogConsoleEvent::Who(None));
    log(&mut app, LogConsoleEvent::Search("MEMORY".into()));
    let (text, _) = render(&app);
    assert!(!text.contains("Step started"), "{text}");
    assert!(text.contains("out of memory"), "{text}");
    log(&mut app, LogConsoleEvent::Search("nothing like it".into()));
    let (text, _) = render(&app);
    assert!(text.contains("No line matches the filter."), "{text}");
    log(&mut app, LogConsoleEvent::Clear);
    assert!(app.env.log.since(0).is_empty(), "the buffer is cleared too");
    assert_eq!(app.env.log.calls_since(0).len(), 1, "the calls stay");
    let (text, _) = render(&app);
    assert!(text.contains("Nothing is logged yet."), "{text}");
}

#[test]
fn an_open_line_shows_whole_below_and_leads_to_its_model_call() {
    let mut app = open_with_lines("log-detail");
    log(&mut app, LogConsoleEvent::SelectLine(Some(2)));
    let (text, _) = render(&app);
    for expected in ["inference::llm::call_log", "Show Model Call", "Copy"] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    log(&mut app, LogConsoleEvent::ShowCall("7-3".into()));
    assert_eq!(app.console.view, ConsoleView::Calls);
    let (text, actions) = render(&app);
    assert!(actions.is_empty(), "{actions:?}");
    for expected in [
        "words, batch 1 of 2",
        "sonnet · call 7-3 · 4.2 s",
        "Answer",
        "{\"lines\": []}",
        "Message",
        "System prompt",
        "Schema",
        "1 call",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(
        !text.contains("RULES TEXT"),
        "the prompt waits folded: {text}"
    );
    log(&mut app, LogConsoleEvent::Clear);
    assert!(app.env.log.calls_since(0).is_empty());
    assert_eq!(app.env.log.since(0).len(), 4, "the lines stay");
    let (text, _) = render(&app);
    assert!(text.contains("No model calls yet."), "{text}");
}

#[test]
fn lines_logged_while_the_window_is_closed_show_when_it_opens_again() {
    let mut app = open_with_lines("log-reopen");
    app.apply(vec![Action::ShowLog(false)]);
    let (text, _) = render(&app);
    assert!(!text.contains("Open Log File"), "closed: {text}");
    app.env
        .log
        .push(Fresh::new(Level::INFO, "pipeline::runner", "while closed"));
    app.poll();
    assert_eq!(
        app.console.activity.len(),
        4,
        "nothing is read while closed"
    );
    app.apply(vec![Action::ShowLog(true)]);
    assert_eq!(app.console.activity.len(), 5);
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
