//! The Engines tab: the vocal separation, the second speech engine, the language model with its
//! processes at once, Fix It's model with its `claude` calls at once and whether it follows each
//! job, and the shot cut score.
//!
//! **Role:** draw the saved settings of this tab as a form, and turn each change into an `Edit`
//! of the saved settings with that one change.
//!
//! **Position:** drawn by `settings_window` while Engines is open; uses `form`.
//!
//! **Signals and state:** none; reads the borrowed page and returns events.
//!
//! **Invariants:** a list sends its choice at once; a model name kept in `settings.toml` that the
//! model list does not offer is shown as its own choice until another is picked; each model list
//! marks its own default (Sonnet for a run, Opus for Fix It); the steppers send each press at
//! once and a typed number only when it is finite and within 1–16 processes, 1–100 calls or a
//! score of 1–100; the switch sends each click at once; the lists fill their column.

use eframe::egui::{RichText, Ui};
use job_model::job::{Separator, WhisperModel};

use super::form;
use crate::core::ui::palette::palette;
use crate::core::ui::switch::switch;
use crate::settings::events::SettingsEvent;
use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::claude_models::{self, CLAUDE_MODELS};
use crate::settings::models::page::{Field, SettingsPage};
use crate::settings::services::page_editing::{CUT_SCORES, FIX_CALLS, PROCESSES};

/// Draw the Engines tab from `page` and push what the owner asked for onto `events`; while the
/// window is `closing`, a number still being typed is sent.
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
        let current = &saved.language_model.model;
        if let Some(model) = model_choice(ui, "model", current, "sonnet") {
            edit(&|s| s.language_model.model.clone_from(&model));
        }
        form::help(ui, model_help(current));
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
    form::row(ui, "Fix It model", |ui| {
        let current = &saved.language_model.fix_model;
        if let Some(model) = model_choice(ui, "fix model", current, "opus") {
            edit(&|s| s.language_model.fix_model.clone_from(&model));
        }
        form::help(ui, model_help(current));
        form::help(
            ui,
            "Fix It reads the whole video, fixes the flagged lines, and checks each change \
             against what the engines heard.",
        );
        form::field_error(ui, page, Field::FixModel);
    });
    form::row(ui, "Claude calls at once", |ui| {
        let calls = saved.language_model.fix_calls as f64;
        let range = *FIX_CALLS.start() as f64..=*FIX_CALLS.end() as f64;
        if let Some(calls) = form::stepper(ui, "fix calls", calls, range, closing) {
            edit(&|s| s.language_model.fix_calls = calls.round() as usize);
        }
        form::help(
            ui,
            "How many claude calls Fix It makes at once across every video it fixes (1–100). \
             The rest wait their turn; videos started first go first.",
        );
        form::field_error(ui, page, Field::FixCalls);
    });
    form::row(ui, "Fix It after each job", |ui| {
        // The switch sits level with the label's middle.
        ui.add_space(4.5);
        let on = saved.language_model.fix_after_run;
        if switch(ui, on, "Fix It after each job").clicked() {
            edit(&|s| s.language_model.fix_after_run = !on);
        }
        form::help(
            ui,
            "Fix It starts on each video when its job finishes, if it has lines to fix.",
        );
        form::field_error(ui, page, Field::FixAfterRun);
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

/// A list of the `claude` models with `current` shown, `default` marked, and a name the list
/// lacks as its own choice; the model chosen, when it differs.
fn model_choice(ui: &mut Ui, id: &str, current: &String, default: &str) -> Option<String> {
    let mut options: Vec<(String, String)> = CLAUDE_MODELS
        .iter()
        .map(|m| {
            let tag = if m.name == default { "default" } else { m.tag };
            let label = if tag.is_empty() {
                m.label.to_string()
            } else {
                format!("{} ({tag})", m.label)
            };
            (m.name.to_string(), label)
        })
        .collect();
    if claude_models::find(current).is_none() {
        options.push((current.clone(), current.clone()));
    }
    form::choice(ui, id, current, &options, ui.available_width())
}

/// The help line under a model list.
fn model_help(current: &str) -> &'static str {
    claude_models::find(current).map_or("A model name from settings.toml.", |m| m.how)
}

#[cfg(test)]
#[path = "tests/engines_tab.rs"]
mod tests;
