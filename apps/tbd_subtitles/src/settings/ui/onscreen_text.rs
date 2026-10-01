//! Settings for translated Japanese writing in the same job as dialogue subtitles.
//!
//! **Role:** show visual processing, the localized video, translation fallback, reference folders,
//! the sign library with its Clear button, and model readiness.
//!
//! **Position:** drawn by `settings_window` on On-screen Text; uses the shared form controls.
//!
//! **Signals and state:** reads the settings page; sends edits and navigation as settings events.
//!
//! **Invariants:** each edit goes through save-on-change; the view starts no work or downloads;
//! Replace text in the video is off to clicks while translation is off.

use std::path::PathBuf;

use eframe::egui::{RichText, TextEdit, Ui};

use super::super::form;
use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::core::ui::switch::switch;
use crate::settings::events::SettingsEvent;
use crate::settings::models::machine::CheckState;
use crate::settings::models::page::{Field, SettingsPage, SettingsTab};
use crate::settings::services::model_list;

/// What the sign library is, under its size.
const LIBRARY_HELP: &str = "Signs whose English was drawn into the video and read back cleanly. A later \
                            episode showing the same sign starts from its translation and \
                            lettering. Changing a sign's English in Check Text removes it.";

/// What Replace text in the video does, under its switch.
const REPLACE_HELP: &str = "Erase the Japanese and draw the English into a copy of the video, \
                            <name>.localized.mkv, saved beside the original. Its subtitles go in \
                            <name>.localized.ass. The original video is never changed.";

/// Draw the visual settings; every changed value is saved by the application after this frame.
pub(super) fn onscreen_text_ui(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    let settings = &page.saved.onscreen_text;
    form::row(ui, "Translate on-screen text", |ui| {
        ui.add_space(4.5);
        if switch(ui, settings.enabled, "Translate on-screen text").clicked() {
            let mut edited = page.saved.clone();
            edited.onscreen_text.enabled = !settings.enabled;
            events.push(SettingsEvent::Edit(Box::new(edited)));
        }
        form::help(
            ui,
            "Translate Japanese signs, title cards, credits and visible lyrics during each video job.",
        );
        form::help(
            ui,
            "Dialogue, sound cues and tracked English text are saved together in one ASS file. \
             The original video is never changed.",
        );
        form::field_error(ui, page, Field::OnscreenText);
    });
    form::row(ui, "Replace text in the video", |ui| {
        ui.add_space(4.5);
        let replace = settings.localized_video;
        let clicked = ui
            .add_enabled_ui(settings.enabled, |ui| {
                switch(ui, replace, "Replace text in the video").clicked()
            })
            .inner;
        if clicked {
            let mut edited = page.saved.clone();
            edited.onscreen_text.localized_video = !replace;
            events.push(SettingsEvent::Edit(Box::new(edited)));
        }
        form::help(ui, REPLACE_HELP);
        if !settings.enabled {
            form::help(ui, "Turn on Translate on-screen text to use it.");
        }
    });
    form::divider(ui);
    form::row(ui, "Translation", |ui| {
        ui.add_space(5.0);
        ui.label(RichText::new("Local models first").color(palette(ui).text));
        form::help(
            ui,
            "Read and translate locally, using verified reference translations when available. \
             Unreadable text is flagged for Check Text.",
        );
    });
    form::row(ui, "Claude fallback", |ui| {
        ui.add_space(4.5);
        if switch(ui, settings.claude_fallback, "Claude fallback").clicked() {
            let mut edited = page.saved.clone();
            edited.onscreen_text.claude_fallback = !settings.claude_fallback;
            events.push(SettingsEvent::Edit(Box::new(edited)));
        }
        form::help(
            ui,
            "Have your signed-in Claude CLI read and translate every detected text event once \
             from its keyframe; the local models answer whatever it leaves. Uses the shared \
             Claude call limit; no API key is required.",
        );
        if settings.enabled && settings.claude_fallback {
            claude_status(ui, page, events);
        }
    });
    form::row(ui, "Reference subtitles", |ui| {
        let mut path = settings
            .reference_folder
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        if ui
            .add(
                TextEdit::singleline(&mut path)
                    .id_salt("onscreen-reference-folder")
                    .hint_text("Folder path (optional)")
                    .desired_width(f32::INFINITY),
            )
            .changed()
        {
            let mut edited = page.saved.clone();
            edited.onscreen_text.reference_folder = (!path.is_empty()).then(|| PathBuf::from(path));
            events.push(SettingsEvent::Edit(Box::new(edited)));
        }
        form::help(
            ui,
            "Enter a folder containing reference ASS files. Scene and visible text must match \
             before a translation is reused.",
        );
        form::field_error(ui, page, Field::OnscreenText);
    });
    form::row(ui, "Sign library", |ui| sign_library(ui, page, events));
    form::divider(ui);
    form::row(ui, "Models", |ui| model_status(ui, page, events));
}

/// The sign library's size and its Clear button, which asks before anything is removed.
fn sign_library(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    let library = &page.library;
    let held = match library.size {
        _ if library.clearing => "Clearing…".to_string(),
        Some((signs, bytes)) => format!("{signs} signs, {}.", format::size(bytes)),
        None => "Measuring…".to_string(),
    };
    ui.add_space(5.0);
    ui.label(RichText::new(held).color(palette(ui).text));
    form::help(ui, LIBRARY_HELP);
    if let Some(error) = &library.error {
        form::error(ui, error);
    }
    let signs = library.size.map_or(0, |(signs, _)| signs);
    ui.horizontal_wrapped(|ui| {
        if library.confirming {
            ui.label(format!("Remove all {signs} signs?"));
            if Button::new("Clear").primary(true).show(ui).clicked() {
                events.push(SettingsEvent::ClearLibrary);
            }
            if Button::new("Cancel").show(ui).clicked() {
                events.push(SettingsEvent::KeepLibrary);
            }
        } else if Button::new("Clear…")
            .enabled(signs > 0 && !library.clearing)
            .show(ui)
            .clicked()
        {
            events.push(SettingsEvent::AskClearLibrary);
        }
    });
}

fn claude_status(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    if let Some(check) = page.checks.as_ref().and_then(|checks| {
        checks
            .iter()
            .find(|check| check.name == "claude CLI" && check.state != CheckState::Ok)
    }) {
        form::error(
            ui,
            &format!("Claude fallback is unavailable: {}", check.detail),
        );
        form::help(
            ui,
            "Install and sign in to Claude, then check this computer again.",
        );
        if Button::new("This Computer").show(ui).clicked() {
            events.push(SettingsEvent::Open(SettingsTab::ThisComputer));
        }
    }
}

fn model_status(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    let missing = model_list::missing(&page.items);
    if page.download.is_some() {
        form::help(
            ui,
            "Models are downloading. Progress and Stop are available in Models.",
        );
    } else if missing.any() {
        form::error(ui, &missing.headline());
        form::help(ui, "Download the missing files before starting the queue.");
    } else if !page.saved.onscreen_text.enabled {
        form::help(
            ui,
            "Enable on-screen translation to include its models in the download list.",
        );
    } else if page.items.is_empty() {
        form::help(ui, "Open Models to check the files this job needs.");
    } else {
        form::help(
            ui,
            "All models required by the current settings are on disk.",
        );
    }
    ui.horizontal(|ui| {
        if Button::new("Open Models")
            .icon(icons::PACKAGE)
            .show(ui)
            .clicked()
        {
            events.push(SettingsEvent::Open(SettingsTab::Models));
        }
        if missing.any()
            && page.download.is_none()
            && Button::new("Download Missing")
                .icon(icons::DOWNLOAD)
                .primary(true)
                .show(ui)
                .clicked()
        {
            events.push(SettingsEvent::Download);
        }
    });
}

#[cfg(test)]
#[path = "tests/onscreen_text.rs"]
mod tests;
