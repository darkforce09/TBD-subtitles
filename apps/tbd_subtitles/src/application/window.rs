//! One frame of the window: the desktop's colour scheme, the page tabs, the queue on the left, the
//! page on the right, dropped files, then the actions.

use std::time::Duration;

use eframe::egui::{self, Id, Panel, Ui};

use super::{Action, Page, TbdSubtitlesApp, feature_views};
use crate::core::ui::theme;

/// How often the window redraws while a job runs, so its clock and time left move.
const RUNNING_REDRAW: Duration = Duration::from_secs(1);

impl eframe::App for TbdSubtitlesApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.poll();
        theme::follow(ui.ctx(), self.scheme);
        let actions = self.frame_ui(ui);
        self.apply(actions);
    }
}

impl TbdSubtitlesApp {
    /// Draw one frame and return the actions it asks for; changes nothing itself.
    pub(super) fn frame_ui(&self, ui: &mut Ui) -> Vec<Action> {
        let mut actions = Vec::new();
        let dropped = ui.ctx().input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect::<Vec<_>>()
        });
        if !dropped.is_empty() {
            actions.push(Action::QueueVideos(dropped));
        }
        if self.queue.running_job().is_some() {
            ui.ctx().request_repaint_after(RUNNING_REDRAW);
        }
        Panel::top(Id::new("pages")).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (page, name) in [(Page::Jobs, "Jobs"), (Page::Settings, "Settings")] {
                    if ui.selectable_label(self.page == page, name).clicked() {
                        actions.push(Action::ShowPage(page));
                    }
                }
            });
        });
        Panel::left(Id::new("queue"))
            .resizable(true)
            .default_size(360.0)
            .show(ui, |ui| feature_views::queue_ui(ui, self, &mut actions));
        egui::CentralPanel::default().show(ui, |ui| match self.page {
            Page::Settings => feature_views::settings_ui(ui, self, &mut actions),
            Page::Jobs => feature_views::jobs_ui(ui, self, &mut actions),
        });
        actions
    }
}
