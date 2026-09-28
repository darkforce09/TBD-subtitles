//! A job's line review: the flagged lines on the left, the line being edited on the right with
//! its clip, every engine's reading, the text and flags, and Save.
//!
//! **Role:** draw the borrowed review session and the playing clip's newest frame, and turn each
//! click and edit into a `ReviewEvent`.
//!
//! **Position:** called by the application's Jobs page while a job's review is open.
//!
//! **Signals and state:** keeps the clip's texture in egui's memory, uploading each frame once.
//!
//! **Invariants:** nothing is saved from here; Save is disabled while the job runs.

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, Button, Color32, ColorImage, Id, RichText, ScrollArea, TextEdit, TextStyle,
    TextureHandle, TextureOptions, Ui,
};

use crate::core::ui::palette::palette;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::clip::{Frame, Sound};
use crate::line_review::models::session::{ReviewLine, ReviewSession};
use crate::line_review::services::review_editing::EDITABLE_FLAGS;

/// What the review view draws for one frame.
pub(crate) struct ReviewView<'a> {
    pub(crate) session: &'a ReviewSession,
    pub(crate) playing: bool,
    pub(crate) frame: Option<Frame>,
    /// The job runs now, so a correction cannot be saved.
    pub(crate) job_busy: bool,
}

/// Draw the review and push what the owner asked for onto `events`.
pub(crate) fn review_view_ui(ui: &mut Ui, view: &ReviewView<'_>, events: &mut Vec<ReviewEvent>) {
    let session = view.session;
    ui.horizontal(|ui| {
        if ui.button("← Report").clicked() {
            events.push(ReviewEvent::Close);
        }
        ui.heading("Review lines");
        let mut all = session.show_all;
        if ui.checkbox(&mut all, "Show every line").changed() {
            events.push(ReviewEvent::ShowAll(all));
        }
        ui.label(
            RichText::new(format!("{} corrected", session.corrections.lines.len()))
                .color(palette(ui).text2),
        );
    });
    if let Some(notice) = &session.notice {
        ui.label(RichText::new(notice).color(palette(ui).good));
    }
    ui.separator();
    egui::Panel::left(Id::new("review-lines"))
        .resizable(true)
        .default_size(320.0)
        .show(ui, |ui| list_ui(ui, session, events));
    egui::CentralPanel::default().show(ui, |ui| {
        let line = session
            .draft
            .as_ref()
            .and_then(|draft| session.line(&draft.id));
        match line {
            Some(line) => {
                ScrollArea::vertical().show(ui, |ui| detail_ui(ui, view, line, events));
            }
            None => {
                ui.label(RichText::new("Choose a line on the left.").color(palette(ui).text2));
            }
        }
    });
}

fn list_ui(ui: &mut Ui, session: &ReviewSession, events: &mut Vec<ReviewEvent>) {
    let open = session.draft.as_ref().map(|d| d.id.as_str());
    ScrollArea::vertical().show(ui, |ui| {
        let mut any = false;
        for line in session.shown() {
            any = true;
            let corrected = session.correction(&line.id).is_some();
            let text = match session.correction(&line.id) {
                Some(c) => c.text.as_str(),
                None => line.adjudicated.as_str(),
            };
            // The flag mark in the warning colour; the rest in the row's own text colour.
            let (mark, mark_colour) = if corrected {
                ("✎ ", Color32::PLACEHOLDER)
            } else if line.flagged() {
                ("⚠ ", palette(ui).warn)
            } else {
                ("", Color32::PLACEHOLDER)
            };
            let font = TextStyle::Button.resolve(ui.style());
            let mut label = LayoutJob::default();
            label.append(mark, 0.0, TextFormat::simple(font.clone(), mark_colour));
            label.append(
                &format!("{}  {}  {text}", clock(line.start_s), line.id),
                0.0,
                TextFormat::simple(font, Color32::PLACEHOLDER),
            );
            if ui
                .selectable_label(open == Some(line.id.as_str()), label)
                .clicked()
            {
                events.push(ReviewEvent::Open(line.id.clone()));
            }
        }
        if !any {
            ui.label(RichText::new("No flagged lines.").color(palette(ui).text2));
        }
    });
}

fn detail_ui(ui: &mut Ui, view: &ReviewView<'_>, line: &ReviewLine, events: &mut Vec<ReviewEvent>) {
    let session = view.session;
    let Some(draft) = &session.draft else {
        return;
    };
    ui.horizontal(|ui| {
        ui.heading(format!("{}  {}", line.id, clock(line.start_s)));
        if ui.small_button("◀ Previous").clicked() {
            events.push(ReviewEvent::Step { forward: false });
        }
        if ui.small_button("Next ▶").clicked() {
            events.push(ReviewEvent::Step { forward: true });
        }
    });
    for reason in &line.reasons {
        ui.label(RichText::new(reason).color(palette(ui).warn));
    }
    ui.horizontal(|ui| {
        if view.playing {
            if ui.button("■ Stop").clicked() {
                events.push(ReviewEvent::Stop);
            }
        } else {
            if ui
                .button("▶ Play")
                .on_hover_text("The video's sound")
                .clicked()
            {
                events.push(ReviewEvent::Play(Sound::Mix));
            }
            if ui
                .button("▶ Voices only")
                .on_hover_text("The separated vocal stem")
                .clicked()
            {
                events.push(ReviewEvent::Play(Sound::Voices));
            }
        }
    });
    if let Some(frame) = &view.frame {
        picture_ui(ui, frame);
    }
    ui.add_space(6.0);
    ui.label(RichText::new("What was heard").strong());
    egui::Grid::new("readings").num_columns(3).show(ui, |ui| {
        let settled = std::iter::once(("adjudicated", line.adjudicated.as_str()));
        let heard = line
            .hypotheses
            .iter()
            .map(|h| (h.tag.as_str(), h.text.as_str()));
        for (tag, text) in settled.chain(heard) {
            ui.label(RichText::new(reading_name(tag)).color(palette(ui).text2));
            ui.label(text);
            if ui.small_button("Use").clicked() {
                events.push(ReviewEvent::Pick(tag.to_string()));
            }
            ui.end_row();
        }
    });
    ui.add_space(6.0);
    ui.label(RichText::new("Text").strong())
        .on_hover_text("`||` starts another speaker inside the line");
    let mut text = draft.text.clone();
    if ui
        .add(
            TextEdit::multiline(&mut text)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        )
        .changed()
    {
        events.push(ReviewEvent::EditText(text));
    }
    ui.horizontal(|ui| {
        for flag in EDITABLE_FLAGS {
            let mut on = draft.flags.iter().any(|f| f == flag);
            if ui.checkbox(&mut on, flag_name(flag)).changed() {
                let mut flags: Vec<String> = draft
                    .flags
                    .iter()
                    .filter(|f| f.as_str() != flag)
                    .cloned()
                    .collect();
                if on {
                    flags.push(flag.to_string());
                }
                events.push(ReviewEvent::SetFlags(flags));
            }
        }
    });
    ui.horizontal(|ui| {
        let save = ui
            .add_enabled(!view.job_busy, Button::new("Save and time again"))
            .on_disabled_hover_text("The job is running; save once it ends");
        if save.clicked() {
            events.push(ReviewEvent::Save);
        }
        if session.correction(&line.id).is_some() && ui.button("Take the correction back").clicked()
        {
            events.push(ReviewEvent::Revert(line.id.clone()));
        }
    });
}

/// Show `frame`, uploading it as a texture only when it is new.
fn picture_ui(ui: &mut Ui, frame: &Frame) {
    let id = Id::new("review-clip-texture");
    let held: Option<(u32, TextureHandle)> = ui.ctx().data(|d| d.get_temp(id));
    let texture = match held {
        Some((index, texture)) if index == frame.index => texture,
        _ => {
            let image = ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &frame.rgba,
            );
            let texture = ui
                .ctx()
                .load_texture("review-clip", image, TextureOptions::LINEAR);
            ui.ctx()
                .data_mut(|d| d.insert_temp(id, (frame.index, texture.clone())));
            texture
        }
    };
    ui.image((
        texture.id(),
        egui::vec2(frame.width as f32, frame.height as f32),
    ));
}

fn reading_name(tag: &str) -> &str {
    match tag {
        "adjudicated" => "settled",
        "P" => "Parakeet",
        "W" => "Whisper",
        "ALT p" => "Parakeet, voices",
        "ALT w" => "Whisper, voices",
        other => other,
    }
}

fn flag_name(flag: &str) -> &str {
    match flag {
        "SPK" => "new speaker",
        "NARR" => "narrator",
        "LYRIC" => "song lyric",
        "DROP" => "drop the line",
        other => other,
    }
}

/// A video time as `h:mm:ss`.
fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0).floor() as u64;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}
