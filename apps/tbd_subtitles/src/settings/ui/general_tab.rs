//! The General tab: the models and work folders with their sizes, the subtitle format and the
//! glossary.
//!
//! **Role:** draw the saved settings of this tab as a form, and turn each change into an `Edit`
//! of the saved settings with that one change, or a chooser or Open request.
//!
//! **Position:** drawn by `settings_window` while General is open; uses `form`.
//!
//! **Signals and state:** none; reads the borrowed page and returns events.
//!
//! **Invariants:** an edit carries the saved settings with one field changed; a refused edit's
//! error shows under its field, in place of the glossary's help; a folder stays on one line, the
//! home as `~` and cut in the middle; the models folder cannot change while a download runs.

use eframe::egui::Ui;
use job_model::job::OutputFormat;

use super::form;
use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::segmented::segmented;
use crate::settings::events::{PathField, SettingsEvent};
use crate::settings::models::app_settings::{NO_GLOSSARY, ONE_PIECE};
use crate::settings::models::page::{Field, SettingsPage};

/// Draw the General tab from `page` and push what the owner asked for onto `events`.
pub(super) fn general_ui(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    if let Some(reason) = &page.unreadable {
        form::error(ui, reason);
    }
    let saved = &page.saved;
    form::row(ui, "Models folder", |ui| {
        // The download writes into the models folder, so it stays until the download ends.
        let downloading = page.download.is_some();
        let hover = |response: eframe::egui::Response| {
            if downloading {
                response.on_hover_text("Stop the download first")
            } else {
                response
            }
        };
        form::path_line(ui, &page.models_folder, |ui| {
            if saved.models_dir.is_some() {
                let button = Button::new("Default").enabled(!downloading).show(ui);
                if hover(button).clicked() {
                    let mut edited = saved.clone();
                    edited.models_dir = None;
                    events.push(SettingsEvent::Edit(edited));
                }
            }
            let button = Button::new("Choose…").enabled(!downloading).show(ui);
            if hover(button).clicked() {
                events.push(SettingsEvent::Choose(PathField::ModelsFolder));
            }
        });
        form::help(ui, &models_size(page.models_size));
        form::field_error(ui, page, Field::ModelsFolder);
    });
    form::row(ui, "Work folder", |ui| {
        form::path_line(ui, &page.work_folder, |ui| {
            if Button::new("Open").show(ui).clicked() {
                events.push(SettingsEvent::OpenWorkFolder);
            }
            if saved.work_root.is_some() && Button::new("Default").show(ui).clicked() {
                let mut edited = saved.clone();
                edited.work_root = None;
                events.push(SettingsEvent::Edit(edited));
            }
            if Button::new("Choose…").show(ui).clicked() {
                events.push(SettingsEvent::Choose(PathField::WorkFolder));
            }
        });
        form::help(ui, &work_size(page.work_size));
        form::field_error(ui, page, Field::WorkFolder);
    });
    form::divider(ui);
    form::row(ui, "Subtitle format", |ui| {
        let formats = [
            (OutputFormat::Srt, "SRT", None),
            (OutputFormat::Vtt, "WebVTT", None),
            (OutputFormat::Ass, "ASS", None),
        ];
        ui.horizontal(|ui| {
            if let Some(format) = segmented(ui, saved.output_format, &formats) {
                let mut edited = saved.clone();
                edited.output_format = format;
                events.push(SettingsEvent::Edit(edited));
            }
        });
        form::help(
            ui,
            "SRT plays almost everywhere. The file gets the video's name.",
        );
        form::field_error(ui, page, Field::OutputFormat);
    });
    form::row(ui, "Glossary", |ui| glossary_ui(ui, page, events));
}

/// The glossary: the built-in One Piece names, none, or a file, with a chooser for a file; its
/// count of names, or why the last choice was refused.
fn glossary_ui(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    let saved = &page.saved;
    let mut options = vec![
        (ONE_PIECE.to_string(), "One Piece (built in)".to_string()),
        (NO_GLOSSARY.to_string(), "None".to_string()),
    ];
    if ![ONE_PIECE, NO_GLOSSARY].contains(&saved.glossary.as_str()) {
        let name = std::path::Path::new(&saved.glossary)
            .file_name()
            .map_or_else(
                || saved.glossary.clone(),
                |n| n.to_string_lossy().into_owned(),
            );
        options.push((saved.glossary.clone(), name));
    }
    ui.horizontal(|ui| {
        if let Some(glossary) = form::choice(ui, "glossary", &saved.glossary, &options, 200.0) {
            let mut edited = saved.clone();
            edited.glossary = glossary;
            events.push(SettingsEvent::Edit(edited));
        }
        if Button::new("Choose File…").show(ui).clicked() {
            events.push(SettingsEvent::Choose(PathField::GlossaryFile));
        }
    });
    let refused = page.error.as_ref().filter(|e| e.field == Field::Glossary);
    match (refused, page.glossary_names) {
        (Some(error), _) => form::error(ui, &error.message),
        (None, names) => {
            let count = match names {
                Some(0) | None => "No names.".to_string(),
                Some(n) => format!("{n} names."),
            };
            form::help(
                ui,
                &format!(
                    "{count} Names in the glossary are spelled as written and never flagged as \
                     words no engine heard."
                ),
            );
        }
    }
}

/// The models folder's size on disk, or "Measuring…" until it is known.
fn models_size(size: Option<u64>) -> String {
    size.map_or_else(
        || "Measuring…".to_string(),
        |bytes| format!("{} on disk.", format::size(bytes)),
    )
}

/// The work folder's size, once known, and what the folder is for.
fn work_size(size: Option<u64>) -> String {
    let what = "Each video keeps its steps here so a stopped run can continue.";
    size.map_or_else(
        || what.to_string(),
        |bytes| format!("{}. {what}", format::size(bytes)),
    )
}
