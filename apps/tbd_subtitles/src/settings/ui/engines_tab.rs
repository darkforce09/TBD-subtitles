//! The Engines tab: the vocal separation, the second speech engine, the language model with its
//! processes at once, and the shot cut score.
//!
//! **Role:** draw the saved settings of this tab as a form, and turn each change into an `Edit`
//! of the saved settings with that one change.
//!
//! **Position:** drawn by `settings_window` while Engines is open; uses `form`.
//!
//! **Signals and state:** none; reads the borrowed page and returns events.
//!
//! **Invariants:** the model's name is sent on Enter, when its field loses the focus or when the
//! window closes with it typed; the steppers send each press at once and a typed number only when
//! it is finite and within 1–16 processes or a score of 1–100; the lists fill their column.

use eframe::egui::{RichText, Ui};
use job_model::job::{Separator, WhisperModel};

use super::form;
use crate::core::ui::palette::palette;
use crate::settings::events::SettingsEvent;
use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::page::{Field, SettingsPage};
use crate::settings::services::page_editing::{CUT_SCORES, PROCESSES};

/// Draw the Engines tab from `page` and push what the owner asked for onto `events`; while the
/// window is `closing`, a number or name still being typed is sent.
pub(super) fn engines_ui(
    ui: &mut Ui,
    page: &SettingsPage,
    closing: bool,
    events: &mut Vec<SettingsEvent>,
) {
    let saved = &page.saved;
    let mut edit = |change: &dyn Fn(&mut AppSettings)| {
        let mut edited = saved.clone();
        change(&mut edited);
        events.push(SettingsEvent::Edit(edited));
    };
    form::row(ui, "Vocal separation", |ui| {
        let options = [
            (
                Separator::Roformer,
                "Mel-Band RoFormer (default)".to_string(),
            ),
            (Separator::MdxNet, "MDX-Net Voc_FT (fast)".to_string()),
        ];
        let current = saved.engines.separator;
        if let Some(separator) =
            form::choice(ui, "separator", &current, &options, ui.available_width())
        {
            edit(&|s| s.engines.separator = separator);
        }
        form::help(
            ui,
            match current {
                Separator::Roformer => "Cleanest voices; about 1 min 24 s for a 26-minute video.",
                Separator::MdxNet => "Faster and smaller, with more music left in the voices.",
            },
        );
        form::field_error(ui, page, Field::Separator);
    });
    form::row(ui, "Second speech engine", |ui| {
        let options = [
            (
                WhisperModel::LargeV3,
                "Whisper large-v3 (default)".to_string(),
            ),
            (
                WhisperModel::LargeV3Turbo,
                "Whisper large-v3-turbo (fast)".to_string(),
            ),
        ];
        let current = saved.engines.whisper;
        if let Some(whisper) = form::choice(ui, "whisper", &current, &options, ui.available_width())
        {
            edit(&|s| s.engines.whisper = whisper);
        }
        let how = match current {
            WhisperModel::LargeV3 => "Most accurate second opinion.",
            WhisperModel::LargeV3Turbo => "About twice as fast, a little less accurate.",
        };
        form::help(ui, &format!("{how} Parakeet is always the first engine."));
        form::field_error(ui, page, Field::Whisper);
    });
    form::divider(ui);
    form::row(ui, "Language model", |ui| {
        ui.add_space(5.0);
        ui.label(RichText::new("claude CLI").color(palette(ui).text));
        form::help(
            ui,
            "Chooses between what the engines heard and fixes spelling. It never writes words \
             nobody said.",
        );
    });
    form::row(ui, "Model", |ui| {
        let model = &saved.language_model.model;
        if let Some(model) = form::text_field(ui, "model", model, 140.0, closing) {
            edit(&|s| s.language_model.model.clone_from(&model));
        }
        form::field_error(ui, page, Field::Model);
    });
    form::row(ui, "Processes at once", |ui| {
        let processes = saved.language_model.processes as f64;
        let range = *PROCESSES.start() as f64..=*PROCESSES.end() as f64;
        if let Some(processes) = form::stepper(ui, "processes", processes, range, closing) {
            edit(&|s| s.language_model.processes = processes.round() as usize);
        }
        form::help(ui, "How many requests run side by side (1–16).");
        form::field_error(ui, page, Field::Processes);
    });
    form::divider(ui);
    form::row(ui, "Shot cut score", |ui| {
        if let Some(score) = form::stepper(ui, "cut", saved.cut_score, CUT_SCORES, closing) {
            edit(&|s| s.cut_score = score);
        }
        form::help(
            ui,
            "How big a picture change counts as a shot cut. Subtitles avoid crossing cuts. \
             Default 20.",
        );
        form::field_error(ui, page, Field::CutScore);
    });
}
