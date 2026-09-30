//! A finished job's Overview: the file card with Fix It's result, the localized video's card when
//! the job wrote one, the lines card, then the Details
//! and Step times disclosures, in the column the application gives it, 16 px apart; and the head
//! the two cards share.
//!
//! **Role:** present output, dialogue findings, visual findings and measured stage times.
//! **Position:** job report UI below application composition.
//! **Signals and state:** borrowed report, returned review actions.
//! **Invariants:** visual uncertainty links to Check Text and remains separate from dialogue.

use eframe::egui::{Align, Color32, Frame, Label, Layout, Margin, RichText, TextStyle, Ui};

use crate::core::ui::card::card;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::job_report::events::ReportEvent;
use crate::job_report::models::fixing::FixView;
use crate::job_report::models::report::JobReport;
use crate::job_report::ui::file_card::file_card_ui;
use crate::job_report::ui::lines_card::lines_card_ui;
use crate::job_report::ui::localized_card::localized_card_ui;
use crate::job_report::ui::report_details::{details_ui, step_times_ui};

/// The space between a head's mark and its text, and between its title and its line.
const HEAD_GAP: f32 = 12.0;
const HEAD_LINE_GAP: f32 = 3.0;

/// The Overview as the application lends it for one frame.
pub(crate) struct OverviewView<'a> {
    pub(crate) report: &'a JobReport,
    /// The corrections a correction run of the video is putting into the subtitles, while one
    /// waits or runs.
    pub(crate) updating: Option<usize>,
    /// Fix It for this job: hidden, ready, off with why, under way, or its correction run.
    pub(crate) fix: FixView,
    /// egui's time when Fix It finished on this job, while it has just finished: its result
    /// shows it.
    pub(crate) fixed_at: Option<f64>,
}

/// Draw the Overview of `view` and push what the owner asked for onto `events`.
pub(crate) fn overview_ui(ui: &mut Ui, view: &OverviewView<'_>, events: &mut Vec<ReportEvent>) {
    file_card_ui(ui, view, events);
    let localized = view.report.localized.as_ref();
    if let Some(video) = localized.and_then(|localized| localized.video.as_deref()) {
        let subtitles = localized.and_then(|localized| localized.subtitles.as_deref());
        localized_card_ui(ui, video, subtitles, events);
    }
    lines_card_ui(ui, view.report, events);
    if let Some(text) = &view.report.visual {
        let p = palette(ui);
        card(ui, false, |ui| {
            Frame::new()
                .inner_margin(Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 12.0;
                    ui.heading("On-screen text");
                    let replaced = localized
                        .and_then(|localized| localized.replaced)
                        .map_or_else(String::new, |n| format!(" · {n} replaced in the video"));
                    ui.label(format!(
                        "{} detected · {} translated · {} nearby · {} unresolved{replaced}",
                        text.detected, text.translated, text.fallback, text.unresolved
                    ));
                    let seconds: f64 = view
                        .report
                        .steps
                        .iter()
                        .filter(|(step, _)| step.stage() == job_model::StageName::OnscreenText)
                        .map(|(_, measure)| measure.wall_s)
                        .sum();
                    ui.label(format!(
                        "Visual processing: {:.1} min · Combined ASS output",
                        seconds / 60.0
                    ));
                    if text.flagged > 0 {
                        ui.colored_label(
                            p.warn,
                            format!("{} visual items need review", text.flagged),
                        );
                    }
                    if ui.button("Check Text").clicked() {
                        events.push(ReportEvent::CheckText);
                    }
                });
        });
    }
    details_ui(ui, view.report);
    step_times_ui(ui, view.report, events);
}

/// A card's head: a 28 px `glyph` in `colour`, the headline `title` over `line` in grey, and
/// what `aside` draws at the top right.
pub(super) fn head_ui(
    ui: &mut Ui,
    (glyph, colour): (&str, Color32),
    title: &str,
    line: &str,
    aside: impl FnOnce(&mut Ui),
) {
    let p = palette(ui);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = HEAD_GAP;
        ui.label(RichText::new(glyph).font(icons::font(28.0)).color(colour));
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            aside(ui);
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = HEAD_LINE_GAP;
                ui.label(
                    RichText::new(title)
                        .text_style(TextStyle::Heading)
                        .color(p.text),
                );
                ui.add(Label::new(RichText::new(line).color(p.text2)).wrap());
            });
        });
    });
}
