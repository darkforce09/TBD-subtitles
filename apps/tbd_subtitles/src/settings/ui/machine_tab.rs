//! The This Computer tab: each machine check with its mark (✓, ⚠ or ✗), its name and what it
//! found, and Check Again.
//!
//! **Role:** draw the checks from the borrowed page, a spinner on each while they run again; a
//! CUDA runtime that fails links to the Models tab, where it downloads.
//!
//! **Position:** drawn by `settings_window` while This Computer is open.
//!
//! **Signals and state:** none; reads the borrowed page and returns events.
//!
//! **Invariants:** a check that has not run shows as checking, never as passed; a folder a check
//! found stays on one line, the home as `~` and cut in the middle, "in the models folder" when it
//! is inside it.

use std::path::Path;

use eframe::egui::{
    Align, CursorIcon, FontFamily, FontId, Label, Layout, RichText, Sense, Stroke, Ui, vec2,
};

use super::form;
use crate::core::ui::button::Button;
use crate::core::ui::fonts;
use crate::core::ui::icons::{self, StatusIcon, status_icon};
use crate::core::ui::palette::palette;
use crate::settings::events::SettingsEvent;
use crate::settings::models::machine::{Check, CheckState};
use crate::settings::models::page::{SettingsPage, SettingsTab};
use crate::settings::services::system_check::CUDA_RUNTIME;

/// The width of the name column.
const NAME_WIDTH: f32 = 130.0;

/// Draw the This Computer tab from `page` and push what the owner asked for onto `events`.
pub(super) fn machine_ui(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    let p = palette(ui);
    ui.add(
        Label::new(
            RichText::new(
                "What the app needs from this computer. A problem here doesn't stop a video from \
                 starting, but a step may fail.",
            )
            .color(p.text2),
        )
        .wrap(),
    );
    match &page.checks {
        None => {
            ui.horizontal(|ui| {
                if page.checking {
                    status_icon(ui, StatusIcon::Working, 18.0, false);
                }
                ui.label(RichText::new("Checking…").color(p.text2));
            });
        }
        Some(checks) => {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for (index, check) in checks.iter().enumerate() {
                    let last = index + 1 == checks.len();
                    check_ui(ui, check, page, !last, events);
                }
            });
        }
    }
    if Button::new("Check Again")
        .icon(icons::ARROW_CLOCKWISE)
        .enabled(!page.checking)
        .show(ui)
        .clicked()
    {
        events.push(SettingsEvent::CheckAgain);
    }
}

/// One check's row: 9 px above and below, its mark, name and detail, and a line under it when
/// `line_under`.
fn check_ui(
    ui: &mut Ui,
    check: &Check,
    page: &SettingsPage,
    line_under: bool,
    events: &mut Vec<SettingsEvent>,
) {
    let p = palette(ui);
    let checking = page.checking;
    ui.add_space(9.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        let mark = match check.state {
            _ if checking => StatusIcon::Working,
            CheckState::Ok => StatusIcon::Done,
            CheckState::Warning => StatusIcon::Warning,
            CheckState::Failed => StatusIcon::Failed,
        };
        ui.allocate_ui_with_layout(vec2(20.0, 18.0), Layout::left_to_right(Align::Min), |ui| {
            ui.add_space(1.0);
            status_icon(ui, mark, 18.0, false);
        });
        ui.allocate_ui_with_layout(
            vec2(NAME_WIDTH, 18.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(NAME_WIDTH);
                ui.label(
                    RichText::new(check.name)
                        .family(FontFamily::Name(fonts::SEMIBOLD.into()))
                        .color(p.text),
                );
            },
        );
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(1.0);
            match &check.path {
                Some(folder) => folder_detail_ui(ui, check, folder, &page.models_folder),
                None => {
                    let detail = RichText::new(&check.detail).size(12.0).color(p.text2);
                    ui.add(Label::new(detail).wrap());
                }
            }
            if check.name == CUDA_RUNTIME && check.state == CheckState::Failed && !checking {
                let link = ui
                    .add(
                        Label::new(
                            RichText::new("Download it in Models")
                                .size(12.0)
                                .color(p.accent),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_cursor(CursorIcon::PointingHand);
                if link.clicked() {
                    events.push(SettingsEvent::Open(SettingsTab::Models));
                }
            }
        });
    });
    ui.add_space(9.0);
    if line_under {
        let y = ui.cursor().top();
        ui.painter()
            .hline(ui.max_rect().x_range(), y - 0.5, Stroke::new(1.0, p.line));
    }
}

/// A detail that names a folder, on one line: "… in the models folder" when the folder is inside
/// it, else "… under" the folder with the home as `~`, cut in the middle when too long; the whole
/// folder on hover.
fn folder_detail_ui(ui: &mut Ui, check: &Check, folder: &Path, models_folder: &Path) {
    let p = palette(ui);
    let font = FontId::proportional(12.0);
    let inside = !models_folder.as_os_str().is_empty() && folder.starts_with(models_folder);
    let text = if inside {
        format!("{} in the models folder", check.detail)
    } else {
        let lead = format!("{} under ", check.detail);
        let lead_width = ui
            .painter()
            .layout_no_wrap(lead.clone(), font.clone(), p.text2)
            .size()
            .x;
        let room = ui.available_width() - lead_width;
        format!(
            "{lead}{}",
            form::fit_middle(ui, &form::tilde(folder), &font, room)
        )
    };
    ui.add(Label::new(RichText::new(text).font(font).color(p.text2)).truncate())
        .on_hover_text(folder.display().to_string());
}
