//! The Automation tab: the watch folders, each with Remove and a note when it is not found, Add
//! Folder…, and whether Dolphin offers "Generate subtitles" when videos are right-clicked.
//!
//! **Role:** draw the watch folders and the right-click entry from the borrowed page, and turn
//! Remove into an `Edit` of the saved settings without that folder and Add Folder… into a chooser
//! request.
//!
//! **Position:** drawn by `settings_window` while Automation is open; uses `form`.
//!
//! **Signals and state:** none kept; asks the file system whether each watch folder is there as
//! it draws.
//!
//! **Invariants:** an edit carries the saved settings with one folder fewer; a folder that is not
//! there stays listed, marked "Not found"; a folder stays on one line, the home as `~` and cut in
//! the middle; the right-click entry's state is only shown, never changed, from here.

use eframe::egui::{Label, RichText, Ui};

use super::form;
use crate::core::ui::button::Button;
use crate::core::ui::icons::{StatusIcon, status_icon};
use crate::core::ui::palette::palette;
use crate::settings::events::{PathField, SettingsEvent};
use crate::settings::models::page::{Field, RightClickEntry, SettingsPage};

/// What the watch folders do, under the list.
const WATCH_HELP: &str = "While the app is open, every video in these folders and their \
                          subfolders that has no subtitles yet is queued once it has finished \
                          downloading, and the queue starts by itself. Adding a large folder \
                          queues every video in it without subtitles.";

/// Draw the Automation tab from `page` and push what the owner asked for onto `events`.
pub(super) fn automation_ui(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    let saved = &page.saved;
    form::row(ui, "Watch folders", |ui| {
        if saved.watch_folders.is_empty() {
            let p = palette(ui);
            ui.add(Label::new(
                RichText::new("No watch folders.").size(13.0).color(p.text2),
            ));
        }
        for (index, folder) in saved.watch_folders.iter().enumerate() {
            form::path_line(ui, folder, |ui| {
                if Button::new("Remove").show(ui).clicked() {
                    let mut edited = saved.clone();
                    edited.watch_folders.remove(index);
                    events.push(SettingsEvent::Edit(Box::new(edited)));
                }
            });
            if !folder.is_dir() {
                not_found(ui);
            }
        }
        ui.horizontal(|ui| {
            if Button::new("Add Folder…").show(ui).clicked() {
                events.push(SettingsEvent::Choose(PathField::WatchFolder));
            }
        });
        form::help(ui, WATCH_HELP);
        form::field_error(ui, page, Field::WatchFolders);
    });
    form::divider(ui);
    form::row(ui, "Right-click in Dolphin", |ui| {
        right_click_ui(ui, &page.right_click);
    });
}

/// A warning line under a watch folder that is not there, as when its drive is not mounted.
fn not_found(ui: &mut Ui) {
    let p = palette(ui);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        status_icon(ui, StatusIcon::Warning, 14.0, false);
        ui.add(
            Label::new(
                RichText::new("Not found. Nothing in it is queued until it is back.")
                    .size(12.0)
                    .color(p.warn),
            )
            .wrap(),
        );
    });
}

/// Whether Dolphin offers "Generate subtitles": where its entry is, why it is not there yet, or
/// why it could not be written.
fn right_click_ui(ui: &mut Ui, entry: &RightClickEntry) {
    let p = palette(ui);
    match entry {
        RightClickEntry::Installed(path) => {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                status_icon(ui, StatusIcon::Done, 14.0, false);
                ui.add(
                    Label::new(
                        RichText::new(
                            "“Generate subtitles” appears when you right-click videos in Dolphin.",
                        )
                        .size(13.0)
                        .color(p.text),
                    )
                    .wrap(),
                );
            });
            form::path_line(ui, path, |_| {});
        }
        RightClickEntry::NotInstalled => {
            form::help(ui, "Appears after the app is started from its AppImage.");
        }
        RightClickEntry::Failed(reason) => {
            form::error(
                ui,
                &format!("“Generate subtitles” could not be added to Dolphin: {reason}"),
            );
        }
    }
}

#[cfg(test)]
#[path = "tests/automation_tab.rs"]
mod tests;
