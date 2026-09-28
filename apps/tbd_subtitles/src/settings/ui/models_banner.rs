//! The banner under the toolbar while a model is missing: what is missing with Details… and
//! Download; the download's progress with Stop while it runs; and for a moment after it, in
//! green, that every model is on disk.
//!
//! **Role:** draw the banner `model_list::banner` chose across the window, and turn its buttons
//! into `Open(Models)`, `Download` and `StopDownload`.
//!
//! **Position:** called by the application's frame in a panel under the toolbar.
//!
//! **Signals and state:** the height of its words from the last frame lives in egui's memory, so
//! its buttons sit centred beside them.
//!
//! **Invariants:** orange while something is missing or downloads, green when all is on disk;
//! 16 px beside it and 9 px above and below.

use eframe::egui::{Align, Frame, Label, Layout, Margin, RichText, TextStyle, Ui, Vec2, vec2};

use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::fonts;
use crate::core::ui::icons::{self, StatusIcon, status_icon};
use crate::core::ui::palette::palette;
use crate::core::ui::progress;
use crate::settings::events::SettingsEvent;
use crate::settings::models::page::SettingsTab;
use crate::settings::services::model_list::Banner;

/// The widest the download's bar grows.
const BAR_WIDTH: f32 = 420.0;

/// Draw `banner` across the width given and push what the owner asked for onto `events`.
pub(crate) fn models_banner_ui(ui: &mut Ui, banner: &Banner, events: &mut Vec<SettingsEvent>) {
    let p = palette(ui);
    let tint = match banner {
        Banner::AllOnDisk { .. } => p.good_tint,
        Banner::Missing(_) | Banner::Downloading(_) => p.warn_tint,
    };
    Frame::new()
        .fill(p.window.blend(tint))
        .inner_margin(Margin::symmetric(16, 9))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let height_id = ui.id().with("models-banner-height");
            let height = ui
                .ctx()
                .data(|data| data.get_temp::<f32>(height_id))
                .unwrap_or(36.0)
                .max(28.0);
            let size = vec2(ui.available_width(), height);
            let words = ui
                .allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    buttons_ui(ui, banner, events);
                    let rest = vec2(ui.available_width(), height);
                    ui.allocate_ui_with_layout(rest, Layout::left_to_right(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        mark_ui(ui, banner);
                        words_ui(ui, banner)
                    })
                    .inner
                })
                .inner;
            ui.ctx()
                .data_mut(|data| data.insert_temp(height_id, words.y));
        });
}

/// The banner's buttons, from the right.
fn buttons_ui(ui: &mut Ui, banner: &Banner, events: &mut Vec<SettingsEvent>) {
    match banner {
        Banner::Missing(_) => {
            if Button::new("Download")
                .icon(icons::DOWNLOAD)
                .primary(true)
                .show(ui)
                .clicked()
            {
                events.push(SettingsEvent::Download);
            }
            if Button::new("Details…").show(ui).clicked() {
                events.push(SettingsEvent::Open(SettingsTab::Models));
            }
        }
        Banner::Downloading(_) => {
            if Button::new("Stop").show(ui).clicked() {
                events.push(SettingsEvent::StopDownload);
            }
        }
        Banner::AllOnDisk { .. } => {}
    }
}

/// The 20 px mark: an orange warning, the download arrow in the accent, or a green check.
fn mark_ui(ui: &mut Ui, banner: &Banner) {
    match banner {
        Banner::Missing(_) => {
            status_icon(ui, StatusIcon::Warning, 20.0, false);
        }
        Banner::Downloading(_) => {
            ui.label(
                RichText::new(icons::DOWNLOAD)
                    .font(icons::font(20.0))
                    .color(palette(ui).accent),
            );
        }
        Banner::AllOnDisk { .. } => {
            status_icon(ui, StatusIcon::Done, 20.0, false);
        }
    }
}

/// The headline in semibold over a grey line, and the bar while downloading, 5 px apart; their
/// size.
fn words_ui(ui: &mut Ui, banner: &Banner) -> Vec2 {
    let p = palette(ui);
    let (headline, line) = match banner {
        Banner::Missing(missing) => (
            missing.headline(),
            "Videos can't start until they're on disk. Each file downloads once and is checked \
             for damage."
                .to_string(),
        ),
        Banner::Downloading(progress) => (
            format!(
                "Downloading models · {} of {}",
                format::size(progress.done()),
                format::size(progress.size)
            ),
            format!(
                "Now: {}. A stopped download resumes next time.",
                progress.id
            ),
        ),
        Banner::AllOnDisk { .. } => (
            "All models are on disk.".to_string(),
            "Add videos and press Start Queue.".to_string(),
        ),
    };
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 5.0;
        let semibold = eframe::egui::FontFamily::Name(fonts::SEMIBOLD.into());
        ui.label(
            RichText::new(headline)
                .text_style(TextStyle::Body)
                .family(semibold)
                .color(p.text),
        );
        ui.add(Label::new(RichText::new(line).color(p.text2)).wrap());
        if let Banner::Downloading(progress) = banner {
            let share = progress.done() as f32 / progress.size.max(1) as f32;
            progress::bar(ui, share, ui.available_width().min(BAR_WIDTH), false);
        }
    })
    .response
    .rect
    .size()
}
