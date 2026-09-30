//! The selected job's body on the right, by its state: a waiting job's card, a running job's
//! progress card over its stages, a failed job's card over its stages, a cancelled job's card.
//!
//! **Role:** choose the cards for the selected job and hand each its part of the borrowed view.
//!
//! **Position:** called by the application under the detail pane's header for a job that has
//! not finished; draws `job_cards`, `progress_card` and `stage_list`, with the stage rows from
//! `stage_progress`.
//!
//! **Signals and state:** none; reads the borrowed view and returns events.
//!
//! **Invariants:** a finished job draws nothing here, since its report is the application's; a
//! job that failed before its first step shows no stages.

use eframe::egui::Ui;

use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::{JobState, QueueItem};
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::services::stage_progress;
use crate::job_queue::ui::{job_cards, progress_card, stage_list};

/// Draw `item`'s cards for its state.
pub(crate) fn progress_view_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    item: &QueueItem,
    events: &mut Vec<JobQueueEvent>,
) {
    match &item.state {
        JobState::Waiting => job_cards::waiting_ui(ui, view, item, events),
        JobState::Running(progress) => {
            progress_card::progress_card_ui(ui, view, progress);
            let rows = stage_progress::running(progress, view.now);
            stage_list::stage_list_ui(ui, item.id, &rows);
        }
        JobState::Failed(failure) => {
            job_cards::failed_ui(ui, view, item, failure, events);
            if failure.step.is_some() {
                stage_list::stage_list_ui(ui, item.id, &stage_progress::failed(failure));
            }
        }
        JobState::Cancelled { kept_steps } => {
            job_cards::cancelled_ui(ui, view, item, *kept_steps, events);
        }
        JobState::Busy { owner, .. } => job_cards::busy_ui(ui, item, *owner, events),
        JobState::Finished(_) | JobState::FinishedBefore => {}
    }
}
