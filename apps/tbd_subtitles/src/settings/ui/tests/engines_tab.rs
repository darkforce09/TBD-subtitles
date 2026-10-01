use std::path::PathBuf;

use eframe::egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::*;
use crate::core::ui::theme;
use crate::settings::models::page::FieldError;

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
        right_click: Default::default(),
        library: Default::default(),
    }
}

/// The Engines tab of `page` in a window tall enough to show every row. The first frame only
/// installs the window's fonts, which the icons need from the next frame on.
fn harness(page: &SettingsPage) -> Harness<'_, Vec<SettingsEvent>> {
    let mut installed = false;
    let mut harness = Harness::builder()
        .with_size(vec2(660.0, 1200.0))
        .build_ui_state(
            move |ui, events: &mut Vec<SettingsEvent>| {
                if !installed {
                    theme::install(ui.ctx());
                    installed = true;
                    return;
                }
                engines_ui(ui, page, false, events);
            },
            Vec::new(),
        );
    harness.run();
    harness
}

#[test]
fn fix_it_s_calls_at_once_and_its_switch_say_what_they_do() {
    let page = page();
    let harness = harness(&page);
    harness.get_by_label(
        "How many claude calls Fix It makes at once across every video it fixes (1–100). The \
         rest wait their turn; videos started first go first.",
    );
    assert!(
        harness.get_all_by_value("32").next().is_some(),
        "the stepper shows 32 calls"
    );
    harness
        .get_by_label("Fix It starts on each video when its job finishes, if it has lines to fix.");
    assert!(harness.state().is_empty(), "an idle frame asks for nothing");
}

#[test]
fn the_switch_turns_fix_it_after_each_job_over() {
    let mut page = page();
    for on in [false, true] {
        page.saved.language_model.fix_after_run = on;
        let mut harness = harness(&page);
        harness.get_by_label("Fix It after each job").click();
        harness.run();
        let edits: Vec<bool> = harness
            .state()
            .iter()
            .map(|event| match event {
                SettingsEvent::Edit(edited) => edited.language_model.fix_after_run,
                _ => panic!("only an edit"),
            })
            .collect();
        assert_eq!(edits, [!on], "a click turns it over once");
    }
}

#[test]
fn a_refused_number_of_calls_shows_its_error_under_it() {
    let mut page = page();
    page.error = Some(FieldError {
        field: Field::FixCalls,
        message: "Claude calls at once must be 1 to 100. It was not changed.".into(),
    });
    let harness = harness(&page);
    harness.get_by_label("Claude calls at once must be 1 to 100. It was not changed.");
}
