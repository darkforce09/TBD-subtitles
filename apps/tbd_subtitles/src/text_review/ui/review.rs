//! Check Text: an occurrence list beside its English editor and rendered comparison.
//!
//! **Role:** present Japanese observations, translations, provenance and correction controls.
//! **Position:** application view over borrowed text review state; returns feature events.
//! **Signals and state:** only widget memory and bounded preview textures live in egui.
//! **Invariants:** editing clones the draft; saving, undoing and retrying happen in services;
//! no correction is saved while its video is running.

use eframe::egui::{
    self, CentralPanel, CornerRadius, DragValue, Frame, Id, Margin, Panel, RichText, ScrollArea,
    Sense, Stroke, Ui,
};
use job_model::onscreen::{Point, TextEdit, TextOccurrence, TextTreatment};

use super::preview;
use crate::core::format;
use crate::core::ui::button::Button;
use crate::core::ui::palette::palette;
use crate::text_review::models::{Comparison, Event, Session};

pub(crate) fn show(
    ui: &mut Ui,
    session: &Session,
    comparison: Option<&Comparison>,
    playing: bool,
    busy: bool,
    events: &mut Vec<Event>,
) {
    let p = palette(ui);
    let list_width = (ui.available_width() * 0.3).clamp(240.0, 330.0);
    Panel::left(Id::new("text-review-list"))
        .exact_size(list_width)
        .resizable(false)
        .frame(Frame::new().fill(p.window))
        .show(ui, |ui| list(ui, session, busy, events));
    CentralPanel::default()
        .frame(Frame::new().fill(p.grouped).inner_margin(16))
        .show(ui, |ui| {
            ScrollArea::vertical()
                .id_salt("text-review-editor")
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if let Some(error) = &session.error {
                        ui.colored_label(p.bad, error);
                        ui.add_space(8.0);
                    }
                    if let Some(text) = session.document.occurrences.get(session.selected) {
                        ui.push_id(&text.id, |ui| {
                            editor(ui, session, text, comparison, playing, busy, events);
                        });
                    } else {
                        ui.heading("No on-screen writing to review");
                        ui.label(
                            RichText::new("Text found during this video's run appears here.")
                                .color(p.text2),
                        );
                    }
                });
        });
}

fn list(ui: &mut Ui, session: &Session, busy: bool, events: &mut Vec<Event>) {
    let p = palette(ui);
    let summary = session.document.summary();
    Frame::new().inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.label(RichText::new("On-screen text").strong().size(14.0));
        ui.label(
            RichText::new(format!(
                "{} found · {} to check",
                summary.detected, summary.flagged
            ))
            .size(12.0)
            .color(p.text2),
        );
        let mut flagged = session.flagged_only;
        for warning in &session.document.review_warnings {
            ui.colored_label(p.warn, warning);
        }
        if !session.document.review_warnings.is_empty()
            && Button::new("Discard unmatched corrections")
                .enabled(!busy)
                .show(ui)
                .clicked()
        {
            events.push(Event::DiscardOrphans);
        }
        if ui
            .checkbox(&mut flagged, "Show flagged text only")
            .changed()
        {
            events.push(Event::FlaggedOnly(flagged));
        }
    });
    ui.separator();
    ScrollArea::vertical()
        .id_salt("text-occurrences")
        .auto_shrink(false)
        .show(ui, |ui| {
            let mut shown = 0;
            for (index, text) in session.document.occurrences.iter().enumerate() {
                let flagged = text.rendered == Some(false)
                    || (!text.reviewed && (text.english.is_none() || !text.warnings.is_empty()));
                if session.flagged_only && !flagged && index != session.selected {
                    continue;
                }
                shown += 1;
                occurrence_row(ui, session, index, text, events);
            }
            if shown == 0 {
                Frame::new().inner_margin(12).show(ui, |ui| {
                    ui.label(
                        RichText::new(if session.flagged_only {
                            "Nothing left to check. Turn off the filter to see every translation."
                        } else {
                            "No text occurrences found."
                        })
                        .color(p.text2),
                    );
                });
            }
        });
}

fn occurrence_row(
    ui: &mut Ui,
    session: &Session,
    index: usize,
    text: &TextOccurrence,
    events: &mut Vec<Event>,
) {
    let p = palette(ui);
    let selected = index == session.selected;
    let response = Frame::new()
        .fill(if selected {
            p.selection_fill()
        } else {
            p.window
        })
        .inner_margin(Margin::symmetric(10, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                preview::thumbnail(ui, session, index);
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(format::clock_tenths(text.start_s))
                            .strong()
                            .size(12.0),
                    );
                    ui.label(
                        RichText::new(if text.english.is_none() || text.rendered == Some(false) {
                            "Unresolved"
                        } else if text.reviewed {
                            "Reviewed"
                        } else if !text.warnings.is_empty() {
                            "To check"
                        } else {
                            "Translated"
                        })
                        .size(11.0)
                        .color(if text.rendered == Some(false) {
                            p.warn
                        } else if text.reviewed && text.english.is_some() {
                            p.good
                        } else {
                            p.text2
                        }),
                    );
                });
            });
            ui.add(
                egui::Label::new(RichText::new(&text.japanese).size(12.0).color(p.text2))
                    .truncate(),
            );
            let english = if selected {
                session
                    .draft
                    .as_ref()
                    .map_or(text.english.as_deref(), |draft| draft.english.as_deref())
            } else {
                text.english.as_deref()
            };
            ui.add(
                egui::Label::new(RichText::new(english.unwrap_or("Translation needed")).size(13.0))
                    .truncate(),
            );
            ui.label(
                RichText::new(format!(
                    "{:.0}% confidence · {}",
                    text.confidence * 100.0,
                    if text.provenance.backend.is_empty() {
                        "OCR"
                    } else {
                        &text.provenance.backend
                    }
                ))
                .size(10.5)
                .color(p.text2),
            );
            if let Some(reference) = &text.provenance.reference {
                let name = reference.file_name().unwrap_or(reference.as_os_str());
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("Reference: {}", name.to_string_lossy()))
                            .size(10.5)
                            .color(p.text2),
                    )
                    .truncate(),
                )
                .on_hover_text(reference.display().to_string());
            }
            if !text.warnings.is_empty() && (!text.reviewed || text.rendered == Some(false)) {
                ui.label(
                    RichText::new(format!("{} to review", text.warnings.len()))
                        .size(10.5)
                        .color(p.warn),
                )
                .on_hover_text(text.warnings.join("\n"));
            }
        })
        .response;
    if ui
        .interact(
            response.rect,
            Id::new(("select-text", &text.id)),
            Sense::click(),
        )
        .clicked()
    {
        events.push(Event::Select(index));
    }
    ui.add_space(1.0);
}

fn editor(
    ui: &mut Ui,
    session: &Session,
    text: &TextOccurrence,
    comparison: Option<&Comparison>,
    playing: bool,
    busy: bool,
    events: &mut Vec<Event>,
) {
    let p = palette(ui);
    ui.horizontal_wrapped(|ui| {
        ui.heading(format!(
            "{} – {}",
            format::clock_tenths(text.start_s),
            format::clock_tenths(text.end_s)
        ));
        ui.label(RichText::new(&text.id).size(11.0).color(p.text2));
        if text.rendered == Some(false) {
            ui.colored_label(p.warn, "Not rendered");
        } else if text.reviewed {
            ui.colored_label(p.good, "Reviewed");
        }
    });
    ui.add_space(8.0);
    preview::show(ui, session, comparison, playing, busy, events);
    ui.add_space(12.0);
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(14)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Japanese").size(11.0).color(p.text2));
            ui.label(&text.japanese);
            provenance(ui, text);
            for warning in &text.warnings {
                ui.label(RichText::new(warning).size(12.0).color(p.warn));
            }
            let saved = session
                .corrections
                .edits
                .get(&text.id)
                .filter(|edit| edit.matches_source(text))
                .cloned()
                .unwrap_or_else(|| TextEdit::from_occurrence(text));
            let mut draft = session.draft.clone().unwrap_or_else(|| saved.clone());
            let before = draft.clone();
            ui.add_space(8.0);
            ui.add_enabled_ui(!busy, |ui| fields(ui, session, text, &mut draft));
            let valid = draft.validate(session.duration_s);
            if let Err(error) = &valid {
                ui.colored_label(p.bad, error);
            }
            if draft != before {
                events.push(Event::Edit(draft.clone()));
            }
            let dirty = draft != saved;
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if Button::new("Save & regenerate ASS")
                    .primary(true)
                    .enabled(!busy && valid.is_ok())
                    .show(ui)
                    .on_hover_text("Keep this translation and update the subtitle file.")
                    .clicked()
                {
                    events.push(Event::Save);
                }
                if Button::new("Undo")
                    .enabled(!busy && (dirty || session.corrections.edits.contains_key(&text.id)))
                    .show(ui)
                    .on_hover_text("Restore the automatic translation and presentation.")
                    .clicked()
                {
                    events.push(Event::Undo);
                }
                if Button::new("Retry selected text")
                    .enabled(!busy)
                    .show(ui)
                    .on_hover_text("Read and translate this occurrence again.")
                    .clicked()
                {
                    events.push(Event::Retry);
                }
            });
            if busy {
                ui.label(
                    RichText::new("Changes are available when this video's run finishes.")
                        .size(12.0)
                        .color(p.text2),
                );
            } else if dirty {
                ui.label(
                    RichText::new("Unsaved changes. The comparison shows the saved subtitles.")
                        .size(12.0)
                        .color(p.text2),
                );
            }
        });
}

fn provenance(ui: &mut Ui, text: &TextOccurrence) {
    let p = palette(ui);
    ui.label(
        RichText::new(format!(
            "{:.0}% confidence · {}",
            text.confidence * 100.0,
            text.provenance.backend
        ))
        .size(12.0)
        .color(p.text2),
    );
    if let Some(reference) = &text.provenance.reference {
        ui.label(
            RichText::new(format!("Reference: {}", reference.display()))
                .size(11.0)
                .color(p.text2),
        );
    }
    if !text.provenance.reason.is_empty() {
        ui.label(
            RichText::new(&text.provenance.reason)
                .size(12.0)
                .color(p.text2),
        );
    }
}

fn fields(ui: &mut Ui, session: &Session, text: &TextOccurrence, draft: &mut TextEdit) {
    ui.label(RichText::new("English").strong());
    let mut english = draft.english.clone().unwrap_or_default();
    if ui
        .add(
            egui::TextEdit::multiline(&mut english)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("Enter a verified English translation"),
        )
        .changed()
    {
        draft.english = if english.trim().is_empty() {
            None
        } else {
            Some(english)
        };
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("Start");
        ui.add(
            DragValue::new(&mut draft.start_s)
                .speed(0.01)
                .range(0.0..=session.duration_s)
                .max_decimals(3)
                .suffix(" s"),
        );
        ui.label("End");
        ui.add(
            DragValue::new(&mut draft.end_s)
                .speed(0.01)
                .range(0.0..=session.duration_s)
                .max_decimals(3)
                .suffix(" s"),
        );
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Treatment");
        egui::ComboBox::from_id_salt("text-treatment")
            .selected_text(treatment_name(&draft.presentation.treatment))
            .show_ui(ui, |ui| {
                for treatment in [
                    TextTreatment::Auto,
                    TextTreatment::Replace,
                    TextTreatment::Nearby,
                ] {
                    let label = treatment_name(&treatment);
                    ui.selectable_value(&mut draft.presentation.treatment, treatment, label);
                }
            });
    });
    let mut custom_position = draft.presentation.anchor.is_some();
    ui.horizontal_wrapped(|ui| {
        if ui
            .checkbox(&mut custom_position, "Custom position")
            .changed()
        {
            draft.presentation.anchor = custom_position.then(|| {
                text.frames
                    .first()
                    .map(|frame| frame.quad.center())
                    .unwrap_or(Point {
                        x: f64::from(session.document.width) / 2.0,
                        y: f64::from(session.document.height) / 2.0,
                    })
            });
        }
        if let Some(anchor) = &mut draft.presentation.anchor {
            ui.label("X");
            ui.add(
                DragValue::new(&mut anchor.x)
                    .speed(1.0)
                    .range(0.0..=f64::from(session.document.width)),
            );
            ui.label("Y");
            ui.add(
                DragValue::new(&mut anchor.y)
                    .speed(1.0)
                    .range(0.0..=f64::from(session.document.height)),
            );
        }
    });
    let mut custom_size = draft.presentation.font_size.is_some();
    ui.horizontal_wrapped(|ui| {
        if ui.checkbox(&mut custom_size, "Custom size").changed() {
            draft.presentation.font_size = custom_size.then_some(48.0);
        }
        if let Some(size) = &mut draft.presentation.font_size {
            ui.add(
                DragValue::new(size)
                    .speed(0.5)
                    .range(8.0..=400.0)
                    .suffix(" px"),
            );
        }
    });
}

fn treatment_name(treatment: &TextTreatment) -> &'static str {
    match treatment {
        TextTreatment::Auto => "Automatic",
        TextTreatment::Replace => "Replace",
        TextTreatment::Nearby => "Nearby translation",
    }
}
