//! Draws the queued videos, each with a button that takes it out of the queue.

use eframe::egui::{RichText, ScrollArea, Ui};

use crate::core::ui::MUTED_TEXT;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::view::JobQueueView;

/// Draw the queue from `view` and push what the user asked for onto `events`.
pub(crate) fn queue_panel_ui(
    ui: &mut Ui,
    view: &JobQueueView<'_>,
    events: &mut Vec<JobQueueEvent>,
) {
    ui.heading("Queue");
    ui.horizontal(|ui| {
        if ui.button("Add videos…").clicked() {
            events.push(JobQueueEvent::AddVideos);
        }
        if ui.button("Add folder…").clicked() {
            events.push(JobQueueEvent::AddFolder);
        }
    });
    ui.separator();
    if view.videos.is_empty() {
        ui.label(
            RichText::new("No videos queued. Add videos or a folder, or drop them here.")
                .color(MUTED_TEXT),
        );
        return;
    }
    ScrollArea::vertical().show(ui, |ui| {
        for (index, video) in view.videos.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .small_button("✕")
                    .on_hover_text("Remove from the queue")
                    .clicked()
                {
                    events.push(JobQueueEvent::Remove(index));
                }
                let name = video.file_name().map_or_else(
                    || video.display().to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                ui.label(name).on_hover_text(video.display().to_string());
            });
        }
    });
}
