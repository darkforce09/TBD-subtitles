//! The top of the line editor: the line's time and id with the status chip of its correction
//! run, and why it is worth a listen, or what was done with it.
//!
//! **Role:** draw the head and the why boxes of the open line from the borrowed session.
//!
//! **Position:** drawn by `line_editor` above the clip.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a line Fix It changed and the owner has not checked shows Claude's change
//! first, with what the app had and why, then the other reasons it is worth a listen; a line the
//! owner settled shows what the owner did; the chip names its state in words.

use eframe::egui::{
    Align, Align2, Color32, CornerRadius, FontFamily, FontId, Frame, Label, Layout, Margin, Rect,
    RichText, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use job_model::outputs::Chosen;

use crate::core::format;
use crate::core::ui::fonts;
use crate::core::ui::icons::{self, StatusIcon, paint_status, status_icon};
use crate::core::ui::palette::palette;
use crate::job_report::models::finding_group::LineGroup;
use crate::line_review::models::session::{ReviewLine, ReviewSession, RunState};

/// The line's time, its id and length, and on the right its run's status chip.
pub(super) fn head_ui(ui: &mut Ui, session: &ReviewSession, line: &ReviewLine) {
    let p = palette(ui);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.label(
            RichText::new(format::clock_tenths(line.start_s))
                .font(FontId::new(17.0, FontFamily::Name(fonts::SEMIBOLD.into())))
                .color(p.text),
        );
        ui.label(
            RichText::new(format!("{} · {:.1} s", line.id, line.length_s()))
                .size(12.0)
                .color(p.text2),
        );
        if let Some(state) = session.run_shown() {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                status_chip_ui(ui, state);
            });
        }
    });
}

/// Saved, Updating subtitles…, Subtitles updated, or Subtitles not updated: a 24 px chip, its
/// mark 10 px in and its words 6 px after it, named by its words.
fn status_chip_ui(ui: &mut Ui, state: RunState) {
    let p = palette(ui);
    let (fill, colour, text) = match state {
        RunState::Saved => (p.seg_bg, p.text2, "Saved"),
        RunState::Updating => (p.accent_tint, p.accent, "Updating subtitles…"),
        RunState::Updated => (p.good_tint, p.good, "Subtitles updated"),
        RunState::Failed => (p.seg_bg, p.bad, "Subtitles not updated"),
    };
    let words = ui
        .painter()
        .layout_no_wrap(text.to_string(), FontId::proportional(12.0), colour);
    let size = vec2(10.0 + 14.0 + 6.0 + words.size().x + 10.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    let centre = rect.center().y;
    let mark = Rect::from_center_size(pos2(rect.left() + 17.0, centre), vec2(14.0, 14.0));
    match state {
        RunState::Saved => {
            ui.painter().text(
                mark.center(),
                Align2::CENTER_CENTER,
                icons::CHECK,
                icons::font(14.0),
                colour,
            );
        }
        RunState::Updating => paint_status(ui, mark, StatusIcon::Working, false),
        RunState::Updated => paint_status(ui, mark, StatusIcon::Done, false),
        RunState::Failed => paint_status(ui, mark, StatusIcon::Warning, false),
    }
    let at = pos2(mark.right() + 6.0, centre - words.size().y / 2.0);
    ui.painter().galley(at, words, colour);
}

/// Why the line is worth a listen, a box per group, Claude's change first; or, once the owner
/// settled it, what the owner did.
pub(super) fn why_ui(ui: &mut Ui, session: &ReviewSession, line: &ReviewLine) {
    let p = palette(ui);
    let done = |ui: &mut Ui| status_icon(ui, StatusIcon::Done, 18.0, false);
    match session.correction(&line.id).map(|c| &c.chosen) {
        Some(Chosen::FixIt { model, why }) => {
            let wand = |ui: &mut Ui| {
                ui.label(
                    RichText::new(icons::MAGIC_WAND)
                        .font(icons::font(18.0))
                        .color(p.accent),
                );
            };
            let had = if line.adjudicated.is_empty() {
                "The app had no text here.".to_string()
            } else {
                format!("The app had “{}”.", line.adjudicated)
            };
            let body = format!(
                "{had} {why} Keep Change makes it yours; Undo Change puts the app's reading \
                 back."
            );
            let title = format!(
                "{} changed this line",
                crate::settings::models::claude_models::display_name(model)
            );
            why_box(ui, p.accent_tint, wand, &title, &body);
            for (group, why) in &line.groups {
                if *group != LineGroup::ChangedByFixIt {
                    let warn = |ui: &mut Ui| status_icon(ui, StatusIcon::Warning, 18.0, false);
                    why_box(ui, p.warn_tint, warn, group.title(), why);
                }
            }
        }
        Some(Chosen::KeptFixIt { .. }) => why_box(
            ui,
            p.good_tint,
            done,
            "You kept Claude's change",
            "Its text is yours now, timed again to the audio. Take Back returns it to the app's \
             reading.",
        ),
        Some(_) => {
            let (title, body) = if session.kept(line) {
                (
                    "You kept this line",
                    "Its warnings are cleared and it was timed again.",
                )
            } else {
                (
                    "You corrected this line",
                    "Your text is in the file, timed again to the audio.",
                )
            };
            let body = format!("{body} Take Back returns it to the app's reading.");
            why_box(ui, p.good_tint, done, title, &body);
        }
        None => {
            for (group, why) in &line.groups {
                let warn = |ui: &mut Ui| status_icon(ui, StatusIcon::Warning, 18.0, false);
                why_box(ui, p.warn_tint, warn, group.title(), why);
            }
        }
    }
}

/// A tinted box: the mark `mark` draws, the title in semibold and the body under it.
fn why_box<R>(
    ui: &mut Ui,
    fill: Color32,
    mark: impl FnOnce(&mut Ui) -> R,
    title: &str,
    body: &str,
) {
    let p = palette(ui);
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::symmetric(12, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let _ = mark(ui);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    let semibold = FontId::new(13.0, FontFamily::Name(fonts::SEMIBOLD.into()));
                    ui.label(RichText::new(title).font(semibold).color(p.text));
                    let text = RichText::new(body)
                        .size(12.5)
                        .color(p.text.gamma_multiply(0.85));
                    ui.add(Label::new(text).wrap());
                });
            });
        });
}
