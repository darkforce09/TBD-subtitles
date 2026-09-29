//! The Settings window's five tabs, edits written as they are made, and the models banner,
//! rendered headless.

use super::*;
use crate::settings::models::machine::{Check, CheckState};
use crate::settings::models::page::{DownloadProgress, Field, SettingsTab};

/// Render `app` with the Settings window open on `tab`; the text painted, after checking that an
/// idle frame asks for nothing.
fn render_tab(app: &mut TbdSubtitlesApp, tab: SettingsTab) -> String {
    app.apply(vec![Action::from(SettingsEvent::Open(tab))]);
    let (text, actions) = render(app);
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    text
}

fn assert_shows(text: &str, expected: &[&str]) {
    for expected in expected {
        assert!(text.contains(expected), "{expected} not in {text}");
    }
}

#[test]
fn each_settings_tab_shows_its_settings_under_the_tab_bar_and_over_the_footer() {
    let mut app = app("settings", Vec::new());
    let (text, _) = render(&app);
    assert!(!text.contains("Models folder"), "closed at first: {text}");
    app.apply(vec![Action::ShowSettings(true)]);
    assert_eq!(app.settings_window, Some(SettingsTab::General));
    let (text, actions) = render(&app);
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    assert_shows(
        &text,
        &[
            "General",
            "Automation",
            "Engines",
            "Models",
            "This Computer",
            "Changes save as you make them. They apply to videos that haven't started.",
            "Models folder",
            "Choose…",
            "Work folder",
            "Each video keeps its steps here so a stopped run can continue.",
            "Subtitle format",
            "SRT",
            "WebVTT",
            "ASS",
            "SRT plays almost everywhere. The file gets the video's name.",
            "Glossary",
            "One Piece (built in)",
            "Choose File…",
            "names. Names in the glossary are spelled as written",
        ],
    );
    let text = render_tab(&mut app, SettingsTab::Engines);
    assert_shows(
        &text,
        &[
            "Vocal separation",
            "Mel-Band RoFormer (default)",
            "Cleanest voices; about 1 min 24 s for a 26-minute video.",
            "Second speech engine",
            "Whisper large-v3 (default)",
            "Parakeet is always the first engine.",
            "Language model",
            "claude CLI",
            "Model",
            "Claude Sonnet (default)",
            "Processes at once",
            "Fix It model",
            "Claude Opus (default)",
            "Fix It reads the whole video, fixes the flagged lines",
            // The rows below scroll into view; their labels are painted all the same.
            "Claude calls at once",
            "Fix It after each job",
            "Shot cut score",
        ],
    );
    let text = render_tab(&mut app, SettingsTab::Models);
    assert_shows(
        &text,
        &[
            "The list follows your engine choices.",
            "Name",
            "Kind",
            "Status",
            "parakeet-tdt-0.6b-v2",
            "CUDA and cuDNN (13 archives)",
            "onnxruntime",
            "Missing",
            "Download Missing (",
        ],
    );
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    let text = render_tab(&mut app, SettingsTab::Models);
    assert_shows(&text, &["On disk", "Everything a job needs is on disk."]);
    app.settings.checks = Some(vec![
        Check {
            name: "GPU",
            state: CheckState::Ok,
            detail: "NVIDIA GeForce RTX 3070, driver 615.71.09, 6566 of 8192 MiB free".into(),
            path: None,
        },
        Check {
            name: "CUDA runtime",
            state: CheckState::Failed,
            detail: "no CUDA runtime in the runtime folder".into(),
            path: None,
        },
    ]);
    let text = render_tab(&mut app, SettingsTab::ThisComputer);
    assert_shows(
        &text,
        &[
            "What the app needs from this computer.",
            "GPU",
            "6566 of 8192 MiB free",
            "CUDA runtime",
            "Download it in Models",
            "Check Again",
        ],
    );
    app.apply(vec![Action::ShowSettings(true)]);
    assert_eq!(
        app.settings_window,
        Some(SettingsTab::ThisComputer),
        "asking again keeps the open tab"
    );
    app.apply(vec![Action::ShowSettings(false)]);
    assert_eq!(app.settings_window, None);
}

#[test]
fn an_edit_is_written_at_once() {
    let mut app = app("edit", Vec::new());
    let mut edited = app.settings.saved.clone();
    edited.cut_score = 33.0;
    app.apply(vec![Action::from(SettingsEvent::Edit(Box::new(edited)))]);
    assert_eq!(app.settings.saved.cut_score, 33.0);
    assert!(app.settings.error.is_none());
    let written = std::fs::read_to_string(&app.env.settings_path).expect("the settings file");
    assert!(written.contains("cut_score = 33"), "{written}");
}

#[test]
fn a_bad_glossary_is_not_written_and_names_its_field() {
    let mut app = app("bad-glossary", Vec::new());
    let before = std::fs::read_to_string(&app.env.settings_path).expect("scratch settings");
    let mut edited = app.settings.saved.clone();
    edited.glossary = "/no/such/names.json".into();
    app.apply(vec![Action::from(SettingsEvent::Edit(Box::new(edited)))]);
    assert_eq!(
        std::fs::read_to_string(&app.env.settings_path).expect("unchanged"),
        before
    );
    assert_eq!(app.settings.saved.glossary, "one_piece");
    let error = app.settings.error.clone().expect("an error");
    assert_eq!(error.field, Field::Glossary);
    app.apply(vec![Action::ShowSettings(true)]);
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &["Can't use names.json: ", "The glossary was not changed."],
    );
}

#[test]
fn the_banner_says_what_is_missing_and_details_opens_the_models_tab() {
    let mut app = app("banner", Vec::new());
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &[
            "8 models and 2 runtime libraries are missing (",
            "Videos can't start until they're on disk. Each file downloads once and is checked \
             for damage.",
            "Details…",
            "Download",
        ],
    );
    app.apply(vec![Action::from(SettingsEvent::Open(SettingsTab::Models))]);
    assert_eq!(app.settings_window, Some(SettingsTab::Models));
}

#[test]
fn a_download_shows_its_progress_and_then_briefly_that_all_is_on_disk() {
    let mut app = app("banner-download", Vec::new());
    let size = crate::settings::services::model_list::missing(&app.settings.items).bytes;
    app.settings.download = Some(DownloadProgress {
        id: "parakeet-tdt-0.6b-v2".into(),
        held: 512 * 1_048_576,
        total: 2355 * 1_048_576,
        finished: 712 * 1_048_576,
        size,
    });
    let (text, actions) = render(&app);
    assert!(actions.is_empty(), "an idle frame asks for nothing");
    assert_shows(
        &text,
        &[
            "Downloading models · 1.2 GiB of ",
            "Now: parakeet-tdt-0.6b-v2. A stopped download resumes next time.",
            "Stop",
        ],
    );
    assert!(!text.contains("are missing"), "{text}");
    app.settings.download = None;
    app.settings.items.iter_mut().for_each(|i| i.present = true);
    app.settings.downloaded_at = Some(Instant::now());
    let (text, _) = render(&app);
    assert_shows(
        &text,
        &[
            "All models are on disk.",
            "Add videos and press Start Queue.",
        ],
    );
}

#[test]
fn folders_read_from_home_on_one_line_and_a_runtime_in_the_models_folder_says_so() {
    let mut app = app("paths", Vec::new());
    let home = PathBuf::from(std::env::var_os("HOME").expect("a home folder"));
    app.settings.models_folder = home.join(
        "a/very/long/folder/of/folders/that/goes/on/and/on/and/on/until/it/cannot/fit/models",
    );
    let text = render_tab(&mut app, SettingsTab::General);
    assert_shows(&text, &["~/a/very", "…", "/fit/models"]);
    app.settings.checks = Some(vec![Check {
        name: "CUDA runtime",
        state: CheckState::Ok,
        detail: "CUDA, cuDNN and ONNX Runtime found".into(),
        path: Some(app.settings.models_folder.join("cuda")),
    }]);
    let text = render_tab(&mut app, SettingsTab::ThisComputer);
    assert_shows(
        &text,
        &["CUDA, cuDNN and ONNX Runtime found in the models folder"],
    );
}

#[test]
fn the_models_folder_stays_while_a_download_runs() {
    let mut app = app("download-folder", Vec::new());
    app.settings.download = crate::settings::services::model_downloads::begun(&app.settings.items);
    let before = app.settings.saved.clone();
    let mut edited = before.clone();
    edited.models_dir = Some(PathBuf::from("/elsewhere"));
    app.apply(vec![Action::from(SettingsEvent::Edit(Box::new(edited)))]);
    assert_eq!(app.settings.saved, before);
    assert_eq!(
        app.settings.error.as_ref().map(|e| e.field),
        Some(Field::ModelsFolder)
    );
}

/// The whole window with the Settings window on Automation, driven through its accessibility
/// tree; its state is the app and the actions each frame asked for. The first frame only installs
/// the window's fonts.
fn automation_harness(
    app: TbdSubtitlesApp,
) -> egui_kittest::Harness<'static, (TbdSubtitlesApp, Vec<Action>)> {
    let mut installed = false;
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1400.0, 1800.0))
        .build_ui_state(
            move |ui, (app, actions): &mut (TbdSubtitlesApp, Vec<Action>)| {
                if !installed {
                    theme::install(ui.ctx());
                    theme::follow(ui.ctx(), app.scheme);
                    installed = true;
                    return;
                }
                actions.extend(app.frame_ui(ui));
            },
            (app, Vec::new()),
        );
    harness.run();
    harness
}

#[test]
fn the_automation_tab_lists_the_watch_folders_and_edits_them() {
    use egui_kittest::kittest::Queryable as _;
    let mut app = app("automation", Vec::new());
    let here = std::env::temp_dir();
    let gone = PathBuf::from("/no/such/drive/videos");
    let mut edited = app.settings.saved.clone();
    edited.watch_folders = vec![here.clone(), gone.clone()];
    app.apply(vec![Action::from(SettingsEvent::Edit(Box::new(edited)))]);
    let text = render_tab(&mut app, SettingsTab::Automation);
    assert_shows(
        &text,
        &[
            "Watch folders",
            &here.display().to_string(),
            &gone.display().to_string(),
            "Not found. Nothing in it is queued until it is back.",
            "Remove",
            "Add Folder…",
            "While the app is open, every video in these folders and their subfolders",
            "Right-click in Dolphin",
            "Appears after the app is started from its AppImage.",
        ],
    );
    let saved = app.settings.saved.clone();
    let mut harness = automation_harness(app);
    harness
        .get_all_by_label("Remove")
        .nth(1)
        .expect("a Remove for the second folder")
        .click();
    harness.run();
    let mut without = saved.clone();
    without.watch_folders = vec![here];
    assert_eq!(
        harness.state().1,
        [Action::Settings(SettingsEvent::Edit(Box::new(without)))],
        "Remove edits out its own folder"
    );
    harness.state_mut().1.clear();
    // The toolbar and the empty queue offer Add Folder… too; the Automation tab's is the first
    // one under its last Remove.
    let last_remove = harness
        .get_all_by_label("Remove")
        .map(|node| node.rect().bottom())
        .fold(f32::MIN, f32::max);
    harness
        .get_all_by_label("Add Folder…")
        .filter(|node| node.rect().top() >= last_remove)
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the Automation tab offers Add Folder…")
        .click();
    harness.run();
    assert_eq!(
        harness.state().1,
        [Action::Settings(SettingsEvent::Choose(
            crate::settings::events::PathField::WatchFolder
        ))]
    );
}

#[test]
fn a_chosen_watch_folder_is_added_once() {
    use crate::settings::events::PathField;
    let mut app = app("watch-chosen", Vec::new());
    let folder = app
        .env
        .settings_path
        .parent()
        .expect("config")
        .to_path_buf();
    let canonical = std::fs::canonicalize(&folder).expect("canonical");
    app.chosen_setting(PathField::WatchFolder, folder.clone());
    app.chosen_setting(PathField::WatchFolder, folder);
    let gone = PathBuf::from("/no/such/drive/videos");
    app.chosen_setting(PathField::WatchFolder, gone.clone());
    assert_eq!(app.settings.saved.watch_folders, [canonical, gone]);
    assert!(app.settings.error.is_none());
    let written = crate::settings::services::settings_file::load(&app.env.settings_path)
        .expect("the settings file");
    assert_eq!(written.watch_folders, app.settings.saved.watch_folders);
}
