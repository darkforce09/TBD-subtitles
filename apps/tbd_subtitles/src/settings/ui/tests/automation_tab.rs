use std::path::PathBuf;

use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::*;
use crate::core::ui::theme;
use crate::settings::models::app_settings::AppSettings;

fn page() -> SettingsPage {
    SettingsPage {
        path: PathBuf::from("/settings/settings.toml"),
        saved: AppSettings::default(),
        error: None,
        unreadable: None,
        glossary_names: None,
        items: Vec::new(),
        download: None,
        downloaded_at: None,
        checks: None,
        checking: false,
        models_folder: PathBuf::from("/models"),
        models_size: None,
        work_folder: PathBuf::from("/work"),
        work_size: None,
        right_click: RightClickEntry::NotInstalled,
    }
}

/// The Automation tab of `page`. The first frame only installs the window's fonts, which the
/// icons need from the next frame on.
fn harness(page: &SettingsPage) -> Harness<'_, Vec<SettingsEvent>> {
    let mut installed = false;
    let mut harness = Harness::builder()
        .with_size(vec2(660.0, 800.0))
        .build_ui_state(
            move |ui, events: &mut Vec<SettingsEvent>| {
                if !installed {
                    theme::install(ui.ctx());
                    installed = true;
                    return;
                }
                automation_ui(ui, page, events);
            },
            Vec::new(),
        );
    harness.run();
    harness
}

/// A folder that is there, and one that is not.
fn folders() -> (PathBuf, PathBuf) {
    let here = std::env::temp_dir();
    let gone = PathBuf::from("/no/such/drive/videos");
    (here, gone)
}

#[test]
fn no_folders_say_so_and_add_folder_asks_for_the_chooser() {
    let page = page();
    let mut harness = harness(&page);
    harness.get_by_label("No watch folders.");
    harness.get_by_label(WATCH_HELP);
    harness.get_by_label("Appears after the app is started from its AppImage.");
    assert!(harness.state().is_empty(), "an idle frame asks for nothing");
    harness.get_by_label("Add Folder…").click();
    harness.run();
    assert_eq!(
        harness.state().as_slice(),
        [SettingsEvent::Choose(PathField::WatchFolder)]
    );
}

#[test]
fn a_folder_not_found_is_marked_and_remove_edits_it_out() {
    let (here, gone) = folders();
    let mut page = page();
    page.saved.watch_folders = vec![here.clone(), gone.clone()];
    let mut harness = harness(&page);
    let notes: Vec<_> = harness
        .get_all_by_label("Not found. Nothing in it is queued until it is back.")
        .collect();
    assert_eq!(notes.len(), 1, "only the missing folder is marked");
    assert!(harness.query_by_label("No watch folders.").is_none());
    let removes: Vec<_> = harness.get_all_by_label("Remove").collect();
    assert_eq!(removes.len(), 2, "one Remove per folder");
    removes[1].click();
    harness.run();
    let mut expected = page.saved.clone();
    expected.watch_folders = vec![here];
    assert_eq!(
        harness.state().as_slice(),
        [SettingsEvent::Edit(Box::new(expected))],
        "the saved settings without the second folder"
    );
}

#[test]
fn the_right_click_entry_says_where_it_is_or_why_it_failed() {
    let mut page = page();
    page.right_click =
        RightClickEntry::Installed(PathBuf::from("/menus/tbd-subtitles-generate.desktop"));
    let harness = self::harness(&page);
    harness.get_by_label("“Generate subtitles” appears when you right-click videos in Dolphin.");
    harness.get_by_label("/menus/tbd-subtitles-generate.desktop");
    drop(harness);
    page.right_click = RightClickEntry::Failed("the folder is read-only".into());
    let harness = self::harness(&page);
    harness.get_by_label(
        "“Generate subtitles” could not be added to Dolphin: the folder is read-only",
    );
}
