//! The cards of a selected job that is not running: waiting, busy, failed or cancelled.
//!
//! **Role:** draw a waiting job's place in line, when it starts, its path, Run Next (for a full
//! run) and Remove from List; a busy job's owning process and that it starts once that process
//! ends, with Remove from List; a failed job's stage in plain words, the raw message in a box,
//! the steps it kept and when Try Again starts it, with Try Again and Show in Folder; a cancelled
//! job's kept steps and when Try Again starts it, with Try Again and Remove from List.
//!
//! **Position:** called by `progress_view` for the selected job; the words come from
//! `status_text`.
//!
//! **Signals and state:** none; reads the borrowed view and returns events.
//!
//! **Invariants:** Run Next is off for the job first in line; a failed or cancelled job always
//! says when Try Again would start it.

use eframe::egui::Ui;

use crate::core::format;
use crate::core::steps::{stage_of, step_title};
use crate::core::ui::button::Button;
use crate::core::ui::card::{card, card_head, card_text, well_text};
use crate::core::ui::icons::{self, StatusIcon};
use crate::core::ui::palette::palette;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{Failure, JobKind, Move, QueueItem};
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::status_text;

/// The card of waiting job `item`.
pub(super) fn waiting_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    item: &QueueItem,
    events: &mut Vec<JobQueueEvent>,
) {
    let p = palette(ui);
    let place = status_text::place_in_line(view.queue, item);
    let title = if place == 1 {
        "Next in line".to_string()
    } else {
        format!("{} in line", format::ordinal(place))
    };
    let when = status_text::waiting_start(view.queue, item, place, view.models_missing);
    // Only a full run takes its turn in the queue; a correction run starts on its own.
    let full = item.kind == JobKind::Full;
    let text = if full {
        format!("{when} Drag it in the sidebar to change the order.")
    } else {
        when.to_string()
    };
    card(ui, true, |ui| {
        card_head(ui, StatusIcon::Waiting, &title, |ui| {
            card_text(ui, &text, p.text2);
        });
        well_text(ui, &item.video.display().to_string(), false);
        buttons(ui, |ui| {
            if full {
                let run_next = Button::new("Run Next")
                    .icon(icons::ARROW_LINE_UP)
                    .enabled(place > 1)
                    .show(ui);
                if run_next.clicked() {
                    events.push(JobQueueEvent::Move(item.id, Move::Top));
                }
            }
            remove_ui(ui, item, events);
        });
    });
}

/// The card of job `item`, which failed as `failure` says.
pub(super) fn failed_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    item: &QueueItem,
    failure: &Failure,
    events: &mut Vec<JobQueueEvent>,
) {
    let p = palette(ui);
    let (title, plain) = match failure.step {
        Some(step) => (
            format!("Failed at {}", stage_of(step).title),
            format!("{} stopped with an error.", step_title(step)),
        ),
        None => (
            "Failed before the first step".to_string(),
            "The video could not start.".to_string(),
        ),
    };
    let kept = kept_words(failure.kept_steps, "The ");
    let when = status_text::try_again_start(view.queue, item, view.models_missing);
    card(ui, true, |ui| {
        card_head(ui, StatusIcon::Failed, &title, |ui| {
            card_text(ui, &plain, p.bad);
            card_text(ui, &format!("{kept} {when}"), p.text2);
        });
        well_text(ui, &failure.message, true);
        buttons(ui, |ui| {
            try_again_ui(ui, item, events);
            let folder = Button::new("Show in Folder").icon(icons::FOLDER).show(ui);
            if folder.clicked() {
                events.push(JobQueueEvent::Reveal(item.video.clone()));
            }
        });
    });
}

/// The card of busy job `item`, whose video process `owner` (when known) runs.
pub(super) fn busy_ui(
    ui: &mut Ui,
    item: &QueueItem,
    owner: Option<u32>,
    events: &mut Vec<JobQueueEvent>,
) {
    let p = palette(ui);
    let who = match owner {
        Some(pid) => format!("Process {pid}, outside this window, runs this video."),
        None => "Another process, outside this window, runs this video.".to_string(),
    };
    card(ui, true, |ui| {
        card_head(ui, StatusIcon::Waiting, "Busy", |ui| {
            card_text(
                ui,
                &format!("{who} It starts once that process ends."),
                p.text2,
            );
        });
        well_text(ui, &item.video.display().to_string(), false);
        buttons(ui, |ui| remove_ui(ui, item, events));
    });
}

/// The card of job `item`, cancelled with `kept_steps` finished steps kept.
pub(super) fn cancelled_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    item: &QueueItem,
    kept_steps: usize,
    events: &mut Vec<JobQueueEvent>,
) {
    let p = palette(ui);
    let kept = kept_words(kept_steps, "");
    let when = status_text::try_again_start(view.queue, item, view.models_missing);
    card(ui, true, |ui| {
        card_head(ui, StatusIcon::Cancelled, "Cancelled", |ui| {
            card_text(ui, &format!("{kept} {when}"), p.text2);
        });
        buttons(ui, |ui| {
            try_again_ui(ui, item, events);
            remove_ui(ui, item, events);
        });
    });
}

/// "The 9 finished steps are kept. Try Again continues after them.", with `the` in front; a job
/// that ended before it kept a step may still find steps done by an earlier run.
fn kept_words(kept: usize, the: &str) -> String {
    match kept {
        0 => "Try Again continues after any steps already done.".to_string(),
        1 => format!("{the}1 finished step is kept. Try Again continues after it."),
        kept => format!("{the}{kept} finished steps are kept. Try Again continues after them."),
    }
}

/// A row of buttons, 8 px apart.
fn buttons(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        add(ui);
    });
}

/// The blue Try Again.
fn try_again_ui(ui: &mut Ui, item: &QueueItem, events: &mut Vec<JobQueueEvent>) {
    let again = Button::new("Try Again")
        .icon(icons::ARROW_CLOCKWISE)
        .primary(true)
        .show(ui);
    if again.clicked() {
        events.push(JobQueueEvent::TryAgain(item.id, None));
    }
}

/// The red Remove from List.
fn remove_ui(ui: &mut Ui, item: &QueueItem, events: &mut Vec<JobQueueEvent>) {
    let remove = Button::new("Remove from List")
        .icon(icons::X)
        .danger(true)
        .show(ui);
    if remove.clicked() {
        events.push(JobQueueEvent::Remove(item.id));
    }
}
