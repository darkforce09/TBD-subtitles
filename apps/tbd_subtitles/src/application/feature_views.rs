//! Lends each feature its borrowed view, draws it, and turns its events into actions.

use eframe::egui::Ui;

use super::{Action, TbdSubtitlesApp};
use crate::job_queue::models::view::JobQueueView;
use crate::job_queue::ui::queue_panel_ui;

/// Draw the queue panel and collect its events as actions.
pub(super) fn queue_ui(ui: &mut Ui, app: &TbdSubtitlesApp, actions: &mut Vec<Action>) {
    let view = JobQueueView { videos: &app.queue };
    let mut events = Vec::new();
    queue_panel_ui(ui, &view, &mut events);
    actions.extend(events.into_iter().map(Action::from));
}
