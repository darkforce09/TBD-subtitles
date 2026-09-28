//! Check Lines rendered headless: the list, the editor, the footer's states, the status chip of a
//! correction run, the group filter from the Overview, the all-done state and the shortcuts.

use std::path::Path;

use job_model::outputs::{Chosen, Corrections};
use job_model::report::{QcCheck, QcFinding};

use super::*;
use crate::job_report::events::{LinesToCheck, ReportEvent};
use crate::job_report::models::finding_group::LineGroup;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::session::LineList;

/// A finished job of three lines: U1 unsure, U2 settled, U3 with a heard word replaced; the
/// window shows its Check Lines.
fn reviewing(name: &str) -> (TbdSubtitlesApp, PathBuf) {
    let root = std::env::temp_dir().join(format!("tbd-app-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("root");
    let video = root.join("Dressrosa 14.mp4");
    std::fs::write(&video, b"video").expect("video");
    let mut app =
        TbdSubtitlesApp::new(Environment::scratch(&root, stand_in()), vec![video.clone()]);
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.apply(vec![Action::from(JobQueueEvent::Start)]);
    settle(&mut app);
    let job = work_dir(&root, &video);
    std::fs::create_dir_all(&job).expect("job");
    let line = |id: &str, start: f64, p: &str, w: &str| {
        format!(
            r#"{{"id":"{id}","start_s":{start},"end_s":{},"words":[],"locked":[],"line":"{id}","hypotheses":[["P",["{p}"]],["W",["{w}"]]]}}"#,
            start + 1.5
        )
    };
    let sheet = format!(
        "[{},{},{}]",
        line("U1", 10.0, "blame!", "flavor!"),
        line("U2", 12.0, "Go!", "Go!"),
        line("U3", 14.0, "Frankie!", "Frankie!")
    );
    let adjudicated = r#"{"lines":[{"id":"U1","t":"Blaver!","f":["UNSURE"]},{"id":"U2","t":"Go!","f":[]},{"id":"U3","t":"Franky!","f":[]}],
        "findings":{"missing_ids":[],"duplicate_ids":[],"unknown_ids":[],"novel":[],"removed_locked":[],"too_fast":[]},
        "calls":1,"input_tokens":0,"output_tokens":0,"cost_usd":0.0}"#;
    std::fs::write(job.join("sheet.json"), sheet).expect("sheet");
    std::fs::write(job.join("adjudicated.json"), adjudicated).expect("adjudicated");
    write_qc(&job, &["U1", "U3"]);
    let id = app.queue.items[0].id;
    app.apply(vec![
        Action::from(JobQueueEvent::Select(id)),
        Action::ShowTab(DetailTab::CheckLines),
    ]);
    (app, root)
}

/// Write the job's `qc.json` with the findings of `lines` only: U1 unsure, U3 a heard word
/// replaced. A correction run drops the findings of the lines it settles, as
/// `crates/pipeline/src/tasks/layout.rs` does.
fn write_qc(job: &Path, lines: &[&str]) {
    let finding = |check, id: &str, detail: &str| QcFinding {
        check,
        time_s: 10.0,
        text: String::new(),
        detail: detail.into(),
        utterance: Some(id.into()),
    };
    let findings = [
        finding(QcCheck::Unsure, "U1", "U1"),
        finding(QcCheck::RemovedLocked, "U3", "U3: Frankie,"),
    ];
    let qc = QcReport {
        findings: findings
            .into_iter()
            .filter(|f| lines.contains(&f.utterance.as_deref().unwrap_or("")))
            .collect(),
        ..QcReport::default()
    };
    std::fs::write(job.join("qc.json"), serde_json::to_string(&qc).expect("qc")).expect("qc");
}

/// Run a frame per entry of `frames`, each given its events, and return the last frame's actions.
fn frames(
    app: &TbdSubtitlesApp,
    frames: Vec<Vec<egui::Event>>,
    focus: Option<egui::Id>,
) -> Vec<Action> {
    let context = egui::Context::default();
    theme::install(&context);
    let mut actions = Vec::new();
    for (i, events) in frames.into_iter().enumerate() {
        if i == 1
            && let Some(id) = focus
        {
            context.memory_mut(|memory| memory.request_focus(id));
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 1800.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = context.run_ui(input, |ui| actions = app.frame_ui(ui));
        output.textures_delta.clear();
    }
    actions
}

fn work_dir(root: &Path, video: &Path) -> PathBuf {
    root.join("work").join(pipeline::work_dir::job_id(
        &std::fs::canonicalize(video).expect("c"),
    ))
}

fn open(app: &TbdSubtitlesApp) -> Option<String> {
    app.review
        .as_ref()
        .and_then(|(_, session)| session.open.clone())
}

fn review_events(actions: &[Action]) -> Vec<ReviewEvent> {
    actions
        .iter()
        .filter_map(|action| match action {
            Action::Review(event) => Some(event.clone()),
            _ => None,
        })
        .collect()
}

fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

#[test]
fn check_lines_opens_on_the_first_line_to_check() {
    let (app, root) = reviewing("lines-first");
    assert_eq!(open(&app).as_deref(), Some("U1"));
    let (text, actions) = render(&app);
    for expected in [
        "To Check",
        "Checked",
        "All",
        "Search text or time",
        "0:10.0",
        "Blaver!",
        "Unsure",
        "Word replaced",
        "U1 · 1.5 s",
        "Unsure what was said",
        "The engines disagreed and a second listen didn't settle it.",
        "Play",
        "Voices Only",
        "Space plays · the clip includes 0.75 s before and after",
        "line 0:10.0 – 0:11.5",
        "IN THE SUBTITLES NOW",
        "WHAT WAS HEARD",
        "Language model's pick",
        "In use",
        "Parakeet",
        "blame!",
        "Whisper",
        "flavor!",
        "YOUR TEXT",
        "Type || where a second speaker starts.",
        "Esc leaves the text box.",
        "New speaker",
        "Never merged into the line before; they share a subtitle only with dashes",
        "Narrator",
        "In italics, and never in a two-speaker subtitle",
        "Song lyric",
        "Left out of the dialogue; its song can get a sound cue",
        "Drop the line",
        "Left out of the subtitles",
        "Previous",
        "Next",
        "Looks Right",
        "Ctrl+Enter",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(!text.contains("Go!"), "U2 has nothing to check: {text}");
    assert!(
        actions.is_empty(),
        "an idle frame asks for nothing: {actions:?}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn use_edits_the_line_and_save_moves_on_while_the_subtitles_update() {
    let (mut app, root) = reviewing("lines-save");
    app.apply(vec![Action::from(ReviewEvent::Pick("P".into()))]);
    let (text, _) = render(&app);
    for expected in [
        "Edited, not saved",
        "Discard Edit",
        "Save Correction",
        "Ctrl+S",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(!text.contains("Looks Right"), "{text}");
    app.apply(vec![Action::from(ReviewEvent::Open("U3".into()))]);
    app.apply(vec![Action::from(ReviewEvent::Open("U1".into()))]);
    let (text, _) = render(&app);
    assert!(
        text.contains("Save Correction"),
        "the edit survives: {text}"
    );
    app.apply(vec![Action::from(ReviewEvent::Save)]);
    assert_eq!(open(&app).as_deref(), Some("U3"), "on to the next line");
    let (text, _) = render(&app);
    assert!(text.contains("Updating subtitles…"), "{text}");
    assert!(text.contains("Heard word replaced"), "U3's why: {text}");
    settle(&mut app);
    let (text, _) = render(&app);
    assert!(text.contains("Subtitles updated"), "{text}");
    app.apply(vec![Action::from(ReviewEvent::List(LineList::Checked))]);
    let (text, _) = render(&app);
    for expected in ["You corrected this line", "Corrected", "Take Back"] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn looks_right_keeps_the_language_models_text_until_every_line_is_checked() {
    let (mut app, root) = reviewing("lines-keep");
    app.apply(vec![Action::from(ReviewEvent::LooksRight)]);
    settle(&mut app);
    app.apply(vec![Action::from(ReviewEvent::LooksRight)]);
    settle(&mut app);
    let (_, session) = app.review.as_ref().expect("open");
    let written: Corrections = serde_json::from_str(
        &std::fs::read_to_string(session.work_dir.join("review.json")).expect("review.json"),
    )
    .expect("json");
    assert_eq!(written.lines.len(), 2);
    assert!(
        written
            .lines
            .iter()
            .all(|c| c.chosen == Chosen::Engine("adjudicated".into()))
    );
    let (text, _) = render(&app);
    for expected in [
        "All 2 lines checked",
        "The subtitles are up to date.",
        "Show Checked Lines",
        "Nothing left to check.",
    ] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    app.apply(vec![Action::from(ReviewEvent::List(LineList::Checked))]);
    let (text, _) = render(&app);
    assert!(text.contains("Looks right"), "{text}");
    assert!(text.contains("You kept this line"), "{text}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_group_on_the_overview_opens_check_lines_on_that_group() {
    let (mut app, root) = reviewing("lines-group");
    app.apply(vec![Action::ShowTab(DetailTab::Overview)]);
    app.apply(vec![Action::from(ReportEvent::CheckLines(
        LinesToCheck::Group(LineGroup::HeardWordReplaced),
    ))]);
    let (_, session) = app.review.as_ref().expect("Check Lines is open");
    assert_eq!(session.group, Some(LineGroup::HeardWordReplaced));
    assert_eq!(open(&app).as_deref(), Some("U3"));
    let (text, _) = render(&app);
    for expected in ["Showing", "Heard word replaced", "Franky!"] {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
    assert!(!text.contains("Blaver!"), "U1 is in another group: {text}");
    app.apply(vec![Action::from(ReviewEvent::ClearGroup)]);
    let (text, _) = render(&app);
    assert!(
        text.contains("Blaver!") && !text.contains("Showing"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn an_edit_waits_while_check_lines_is_closed() {
    let (mut app, root) = reviewing("lines-park");
    app.apply(vec![Action::from(ReviewEvent::EditText("Brave!".into()))]);
    app.apply(vec![Action::ShowTab(DetailTab::Overview)]);
    assert!(app.review.is_none());
    app.apply(vec![Action::ShowTab(DetailTab::CheckLines)]);
    let (text, _) = render(&app);
    assert!(
        text.contains("Brave!") && text.contains("Save Correction"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn keys_save_keep_play_and_move_through_the_lines_but_not_while_typing() {
    let (mut app, root) = reviewing("lines-keys");
    let ctrl = egui::Modifiers::CTRL | egui::Modifiers::COMMAND;
    let none = egui::Modifiers::NONE;
    let keys = |app: &TbdSubtitlesApp, event| review_events(&render_with(app, vec![event]).1);
    assert_eq!(
        keys(&app, key(egui::Key::Enter, ctrl)),
        [ReviewEvent::LooksRight]
    );
    assert!(
        keys(&app, key(egui::Key::S, ctrl)).is_empty(),
        "nothing to save"
    );
    assert_eq!(
        keys(&app, key(egui::Key::Space, none)),
        [ReviewEvent::Play(
            crate::line_review::models::clip::Sound::Mix
        )]
    );
    assert_eq!(
        keys(&app, key(egui::Key::ArrowDown, none)),
        [ReviewEvent::Step { forward: true }]
    );
    let (_, actions) = render_with(&app, vec![key(egui::Key::Delete, none)]);
    assert!(
        actions.is_empty(),
        "Delete removes no row here: {actions:?}"
    );
    app.apply(vec![Action::from(ReviewEvent::Pick("P".into()))]);
    assert_eq!(keys(&app, key(egui::Key::S, ctrl)), [ReviewEvent::Save]);
    assert!(keys(&app, key(egui::Key::Enter, ctrl)).is_empty(), "edited");
    // With the text box focused, Space and the arrows are the text box's.
    let actions = frames(
        &app,
        vec![
            Vec::new(),
            Vec::new(),
            vec![key(egui::Key::Space, none), key(egui::Key::ArrowDown, none)],
        ],
        Some(egui::Id::new(("review-line-text", "U1"))),
    );
    assert!(
        review_events(&actions).is_empty(),
        "no key fires while typing: {actions:?}"
    );
    // A button that Tab gave the keyboard's focus takes Space: the toolbar's Add Videos….
    let actions = frames(
        &app,
        vec![
            Vec::new(),
            vec![key(egui::Key::Tab, none)],
            vec![key(egui::Key::Space, none)],
        ],
        None,
    );
    assert!(review_events(&actions).is_empty(), "{actions:?}");
    assert_eq!(actions, [Action::Queue(JobQueueEvent::AddVideos)]);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn used_text_stays_frame_after_frame() {
    let (mut app, root) = reviewing("lines-use");
    app.apply(vec![Action::from(ReviewEvent::Pick("P".into()))]);
    for _ in 0..2 {
        let (_, actions) = render(&app);
        assert!(review_events(&actions).is_empty(), "{actions:?}");
    }
    let (_, session) = app.review.as_ref().expect("open");
    assert_eq!(
        session.drafts.get("U1").map(|d| d.text.as_str()),
        Some("blame!")
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn lines_stay_worth_a_listen_across_their_correction_runs() {
    use crate::line_review::services::line_filter;
    let (mut app, root) = reviewing("lines-settled");
    let id = app.queue.items[0].id;
    let job = work_dir(&root, &root.join("Dressrosa 14.mp4"));
    let counts = |app: &TbdSubtitlesApp| {
        let (_, session) = app.review.as_ref().expect("open");
        let counts = line_filter::counts(session);
        let summary = app.summaries.get(&id).copied().unwrap_or_default();
        (
            counts.to_check,
            counts.checked,
            line_filter::worth(session),
            summary.flagged,
            summary.to_check,
        )
    };
    app.apply(vec![Action::from(ReviewEvent::LooksRight)]);
    write_qc(&job, &["U3"]);
    settle(&mut app);
    assert_eq!(
        counts(&app),
        (1, 1, 2, 2, 1),
        "U1 settled and checked, U3 to check"
    );
    app.apply(vec![Action::from(ReviewEvent::LooksRight)]);
    write_qc(&job, &[]);
    settle(&mut app);
    assert_eq!(counts(&app), (0, 2, 2, 2, 0));
    let (text, _) = render(&app);
    assert!(text.contains("All 2 lines checked"), "{text}");
    app.apply(vec![
        Action::from(ReviewEvent::List(LineList::Checked)),
        Action::from(ReviewEvent::Open("U1".into())),
        Action::from(ReviewEvent::Revert("U1".into())),
    ]);
    assert_eq!(
        counts(&app).0,
        1,
        "taken back, U1 is to check while its run runs"
    );
    settle(&mut app);
    assert_eq!(
        counts(&app),
        (0, 1, 1, 1, 0),
        "its run ended and the check left it unflagged"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_status_chip_follows_its_run_while_check_lines_is_closed() {
    let (mut app, root) = reviewing("lines-chip");
    app.apply(vec![Action::from(ReviewEvent::LooksRight)]);
    app.apply(vec![
        Action::ShowTab(DetailTab::Overview),
        Action::from(ReportEvent::CheckLines(LinesToCheck::Group(
            LineGroup::HeardWordReplaced,
        ))),
    ]);
    let (text, _) = render(&app);
    assert!(text.contains("Updating subtitles…"), "reopened: {text}");
    app.apply(vec![Action::ShowTab(DetailTab::Overview)]);
    settle(&mut app);
    app.apply(vec![Action::ShowTab(DetailTab::CheckLines)]);
    let (text, _) = render(&app);
    assert!(
        text.contains("Subtitles updated"),
        "ended while closed: {text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn taking_back_a_line_without_a_correction_queues_nothing() {
    let (mut app, root) = reviewing("lines-revert-none");
    app.apply(vec![Action::from(ReviewEvent::Revert("U2".into()))]);
    assert_eq!(
        app.queue.items.len(),
        1,
        "no correction run: {:?}",
        app.queue.items
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn take_back_stays_on_its_line_in_check_lines_while_the_subtitles_update() {
    let (mut app, root) = reviewing("lines-take-back");
    let job = work_dir(&root, &root.join("Dressrosa 14.mp4"));
    let id = app.queue.selected.expect("a job is selected");
    app.apply(vec![Action::from(ReviewEvent::Pick("P".into()))]);
    app.apply(vec![Action::from(ReviewEvent::Save)]);
    app.apply(vec![Action::from(ReviewEvent::LooksRight)]);
    settle(&mut app);
    // Taken back from the Checked list, which no longer shows the line: U1 while U3 stays
    // corrected, then U3, the last correction, which removes `review.json`.
    for (line, last) in [("U1", false), ("U3", true)] {
        app.apply(vec![
            Action::from(ReviewEvent::List(LineList::Checked)),
            Action::from(ReviewEvent::Open(line.into())),
            Action::from(ReviewEvent::Revert(line.into())),
        ]);
        assert_eq!(job.join("review.json").exists(), !last, "{line}");
        assert_eq!(open(&app).as_deref(), Some(line), "still on {line}");
        let (text, _) = render(&app);
        assert!(text.contains("Updating subtitles…"), "{line}: {text}");
        settle(&mut app);
        assert_eq!(app.detail_tab(id), DetailTab::CheckLines, "{line}");
        assert_eq!(open(&app).as_deref(), Some(line), "{line} after its run");
        let (text, _) = render(&app);
        for expected in [format!("{line} · 1.5 s"), "Subtitles updated".into()] {
            assert!(text.contains(&expected), "{expected} not in {text}");
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}
