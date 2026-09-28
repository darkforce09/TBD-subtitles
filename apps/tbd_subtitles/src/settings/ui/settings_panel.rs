//! The settings page: the form over `settings.toml`, then the models and the machine checks.
//!
//! **Role:** draw the owner's settings as editable fields from the borrowed page, and turn each
//! change and button into a `SettingsEvent`.
//!
//! **Position:** called by the application's frame when the Settings page is open; draws
//! `machine_panel` below the form.
//!
//! **Signals and state:** none; edits go to a copy of the draft that is sent back whole.
//!
//! **Invariants:** nothing is saved from here: saving is the application's `Save` action.

use eframe::egui::{self, ComboBox, DragValue, Grid, RichText, ScrollArea, TextEdit, Ui};
use job_model::job::{OutputFormat, Separator, WhisperModel};

use super::machine_panel::machine_ui;
use crate::core::ui::palette::palette;
use crate::settings::events::{PathField, SettingsEvent};
use crate::settings::models::app_settings::{AppSettings, NO_GLOSSARY, ONE_PIECE};
use crate::settings::models::page::SettingsPage;

/// Draw the whole settings page and push what the owner asked for onto `events`.
pub(crate) fn settings_page_ui(ui: &mut Ui, page: &SettingsPage, events: &mut Vec<SettingsEvent>) {
    ScrollArea::vertical().show(ui, |ui| {
        ui.heading("Settings");
        ui.label(RichText::new(page.path.display().to_string()).color(palette(ui).text2));
        ui.add_space(6.0);
        let mut draft = page.draft.clone();
        form_ui(ui, &mut draft, page, events);
        if draft != page.draft {
            events.push(SettingsEvent::Edit(draft));
        }
        ui.horizontal(|ui| {
            let edited = page.has_edits();
            if ui.add_enabled(edited, egui::Button::new("Save")).clicked() {
                events.push(SettingsEvent::Save);
            }
            if ui
                .add_enabled(edited, egui::Button::new("Revert"))
                .clicked()
            {
                events.push(SettingsEvent::Revert);
            }
            if edited {
                ui.label(RichText::new("Unsaved changes").color(palette(ui).text2));
            }
        });
        if let Some(notice) = &page.notice {
            ui.label(RichText::new(&notice.text).color(if notice.is_error {
                palette(ui).bad
            } else {
                palette(ui).good
            }));
        }
        ui.add_space(12.0);
        machine_ui(ui, page, events);
    });
}

fn form_ui(
    ui: &mut Ui,
    draft: &mut AppSettings,
    page: &SettingsPage,
    events: &mut Vec<SettingsEvent>,
) {
    Grid::new("settings-form")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Models folder");
            folder_ui(ui, &mut draft.models_dir, PathField::ModelsFolder, events);
            ui.end_row();

            ui.label("Work folder");
            ui.vertical(|ui| {
                folder_ui(ui, &mut draft.work_root, PathField::WorkFolder, events);
                ui.horizontal(|ui| {
                    let size = page
                        .work_size
                        .map_or_else(|| "measuring…".to_string(), crate::core::format::size);
                    ui.label(
                        RichText::new(format!("{}: {size}", page.work_folder.display()))
                            .color(palette(ui).text2),
                    );
                    if ui.small_button("Open").clicked() {
                        events.push(SettingsEvent::OpenWorkFolder);
                    }
                });
            });
            ui.end_row();

            ui.label("Glossary");
            glossary_ui(ui, draft, events);
            ui.end_row();

            ui.label("Vocal separation");
            ComboBox::from_id_salt("separator")
                .selected_text(separator_name(draft.engines.separator))
                .show_ui(ui, |ui| {
                    for s in [Separator::Roformer, Separator::MdxNet] {
                        ui.selectable_value(&mut draft.engines.separator, s, separator_name(s));
                    }
                });
            ui.end_row();

            ui.label("Second speech engine");
            ComboBox::from_id_salt("whisper")
                .selected_text(whisper_name(draft.engines.whisper))
                .show_ui(ui, |ui| {
                    for w in [WhisperModel::LargeV3, WhisperModel::LargeV3Turbo] {
                        ui.selectable_value(&mut draft.engines.whisper, w, whisper_name(w));
                    }
                });
            ui.end_row();

            ui.label("Language model");
            ui.horizontal(|ui| {
                ui.label("claude CLI, model");
                ui.add(TextEdit::singleline(&mut draft.language_model.model).desired_width(90.0));
                ui.label("processes at once");
                ui.add(DragValue::new(&mut draft.language_model.processes).range(1..=16));
            });
            ui.end_row();

            ui.label("Shot cut score");
            ui.add(
                DragValue::new(&mut draft.cut_score)
                    .range(1.0..=100.0)
                    .speed(0.5),
            );
            ui.end_row();

            ui.label("Subtitle format");
            ComboBox::from_id_salt("output-format")
                .selected_text(format_name(draft.output_format))
                .show_ui(ui, |ui| {
                    for f in OutputFormat::ALL {
                        ui.selectable_value(&mut draft.output_format, f, format_name(f));
                    }
                });
            ui.end_row();
        });
}

/// A folder setting: its path or the default, a chooser, and a way back to the default.
fn folder_ui(
    ui: &mut Ui,
    value: &mut Option<std::path::PathBuf>,
    field: PathField,
    events: &mut Vec<SettingsEvent>,
) {
    ui.horizontal(|ui| {
        match value {
            Some(path) => ui.label(path.display().to_string()),
            None => ui.label(RichText::new("the default").color(palette(ui).text2)),
        };
        if ui.small_button("Choose…").clicked() {
            events.push(SettingsEvent::Choose(field));
        }
        if value.is_some() && ui.small_button("Default").clicked() {
            *value = None;
        }
    });
}

fn glossary_ui(ui: &mut Ui, draft: &mut AppSettings, events: &mut Vec<SettingsEvent>) {
    ui.horizontal(|ui| {
        let shown = match draft.glossary.as_str() {
            ONE_PIECE => "One Piece (built in)".to_string(),
            NO_GLOSSARY => "None".to_string(),
            path => path.to_string(),
        };
        ComboBox::from_id_salt("glossary")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut draft.glossary,
                    ONE_PIECE.to_string(),
                    "One Piece (built in)",
                );
                ui.selectable_value(&mut draft.glossary, NO_GLOSSARY.to_string(), "None");
            });
        if ui.small_button("File…").clicked() {
            events.push(SettingsEvent::Choose(PathField::GlossaryFile));
        }
    });
}

fn separator_name(s: Separator) -> &'static str {
    match s {
        Separator::Roformer => "Mel-Band RoFormer (default)",
        Separator::MdxNet => "MDX-Net Voc_FT (fast)",
    }
}

fn whisper_name(w: WhisperModel) -> &'static str {
    match w {
        WhisperModel::LargeV3 => "Whisper large-v3 (default)",
        WhisperModel::LargeV3Turbo => "Whisper large-v3-turbo (fast)",
    }
}

fn format_name(f: OutputFormat) -> &'static str {
    match f {
        OutputFormat::Srt => "SRT (default)",
        OutputFormat::Vtt => "WebVTT",
        OutputFormat::Ass => "ASS",
    }
}
