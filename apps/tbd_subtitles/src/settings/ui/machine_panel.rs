//! The machine part of the settings page: the models and runtime libraries with their download,
//! and the checks of the GPU and the programs a job runs.
//!
//! **Role:** draw each model and runtime archive with its size and state or its progress, the
//! download and stop buttons, and each machine check with its mark and detail.
//!
//! **Position:** called by `settings_panel` below the form.
//!
//! **Signals and state:** none; reads the borrowed settings page and returns events.
//!
//! **Invariants:** a check that has not run shows as checking, never as passed.

use eframe::egui::{Grid, ProgressBar, RichText, Ui};

use crate::core::format;
use crate::core::ui::palette::palette;
use crate::settings::events::SettingsEvent;
use crate::settings::models::machine::{CheckState, ItemKind};
use crate::settings::models::page::SettingsPage;
use crate::settings::services::model_downloads;

/// Draw the models list and the checks from `page`.
pub(crate) fn machine_ui(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    ui.heading("Models and runtime");
    let missing = model_downloads::missing_bytes(&page.items);
    Grid::new("models")
        .num_columns(4)
        .striped(true)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            for (index, item) in page.items.iter().enumerate() {
                ui.label(&item.id);
                ui.label(
                    RichText::new(match item.kind {
                        ItemKind::Model => "model",
                        ItemKind::Runtime => "runtime",
                    })
                    .color(palette(ui).text2),
                );
                ui.label(format::size(item.bytes));
                match page.download.filter(|d| d.index == index) {
                    Some(progress) => {
                        let share = progress.held as f32 / progress.total.max(1) as f32;
                        ui.horizontal(|ui| {
                            ui.add(
                                ProgressBar::new(share)
                                    .desired_width(160.0)
                                    .fill(palette(ui).accent_fill),
                            );
                            ui.label(
                                RichText::new(format!("{} %", (share * 100.0) as u32))
                                    .color(palette(ui).text2),
                            );
                        });
                    }
                    None if item.present => {
                        ui.label(RichText::new("✓ on disk").color(palette(ui).good));
                    }
                    None => {
                        ui.label(RichText::new("missing").color(palette(ui).bad));
                    }
                }
                ui.end_row();
            }
        });
    ui.horizontal(|ui| {
        if page.download.is_some() {
            if ui.button("Stop").clicked() {
                events.push(SettingsEvent::StopDownload);
            }
            ui.label(
                RichText::new("Downloading; a stopped file resumes next time.")
                    .color(palette(ui).text2),
            );
        } else if missing > 0 {
            if ui
                .button(format!("Download missing ({})", format::size(missing)))
                .clicked()
            {
                events.push(SettingsEvent::Download);
            }
        } else {
            ui.label(RichText::new("Everything a job needs is on disk.").color(palette(ui).good));
        }
    });
    ui.add_space(12.0);
    ui.heading("This machine");
    match &page.checks {
        None => {
            ui.label(RichText::new("Checking…").color(palette(ui).text2));
        }
        Some(checks) => {
            Grid::new("checks")
                .num_columns(2)
                .spacing([12.0, 4.0])
                .show(ui, |ui| {
                    for check in checks {
                        let (mark, colour) = match check.state {
                            CheckState::Ok => ("✓", palette(ui).good),
                            CheckState::Warning => ("⚠", palette(ui).warn),
                            CheckState::Failed => ("✗", palette(ui).bad),
                        };
                        ui.label(RichText::new(format!("{mark} {}", check.name)).color(colour));
                        ui.label(&check.detail);
                        ui.end_row();
                    }
                });
        }
    }
    if ui
        .add_enabled(!page.checking, eframe::egui::Button::new("Check again"))
        .clicked()
    {
        events.push(SettingsEvent::CheckAgain);
    }
}
