//! A running job's progress card: what its stage is doing, the step at work ("Now: Language model
//! settles the words · step 9 of 18"), a thick bar of the share done, and the time left beside
//! the time so far.
//!
//! **Role:** draw the running job's headline progress from its step states and the step rates.
//!
//! **Position:** called by `progress_view` above the stage list of a running job.
//!
//! **Signals and state:** none; reads the borrowed view.
//!
//! **Invariants:** until the video's length is known the bar is empty and the time left reads
//! "Working out the time left…"; between two steps the card keeps the last step started, never
//! the shot scan running in the background.

use eframe::egui::{Align2, FontFamily, FontId, RichText, Sense, Ui, vec2};
use job_model::StepName;

use crate::core::format;
use crate::core::steps::{stage_of, step_title};
use crate::core::ui::card::card;
use crate::core::ui::fonts;
use crate::core::ui::palette::palette;
use crate::core::ui::progress::bar;
use crate::job_queue::models::progress::JobProgress;
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::stage_progress::step_number;
use crate::job_queue::services::time_left;

/// The height of the line under the bar: the time left and the time so far.
const META_HEIGHT: f32 = 17.0;

/// Draw the progress card of the running job `progress`.
pub(super) fn progress_card_ui(ui: &mut Ui, view: &JobQueueView<'_>, progress: &JobProgress) {
    let p = palette(ui);
    let so_far = view
        .now
        .saturating_duration_since(progress.started)
        .as_secs_f64();
    let estimate = time_left::estimate(progress, view.rates, view.now);
    let step = progress.shown_step();
    card(ui, true, |ui| {
        ui.spacing_mut().item_spacing.y = 8.0;
        let doing = step.map_or("Starting…", |step| stage_of(step).doing);
        ui.label(
            RichText::new(doing)
                .font(FontId::new(17.0, FontFamily::Name(fonts::SEMIBOLD.into())))
                .color(p.text),
        );
        if let Some(step) = step {
            ui.label(
                RichText::new(format!(
                    "Now: {} · step {} of {}",
                    step_title(step),
                    step_number(step),
                    StepName::ALL.len()
                ))
                .size(12.0)
                .color(p.text2),
            );
        }
        let width = ui.available_width();
        bar(
            ui,
            estimate.map_or(0.0, |(_, share)| share as f32),
            width,
            true,
        );
        let left = estimate.map_or_else(
            || "Working out the time left…".to_string(),
            |(left, _)| format!("{} left", format::about(left)),
        );
        let (meta, _) = ui.allocate_exact_size(vec2(width, META_HEIGHT), Sense::hover());
        let font = FontId::proportional(12.0);
        ui.painter().text(
            meta.left_center(),
            Align2::LEFT_CENTER,
            left,
            font.clone(),
            p.text2,
        );
        let so_far = format!("{} so far", format::duration(so_far));
        ui.painter().text(
            meta.right_center(),
            Align2::RIGHT_CENTER,
            so_far,
            font,
            p.text2,
        );
    });
}
