//! One frame of the window: the page tabs, the queue on the left, the page on the right, dropped
//! files, then the actions.

use eframe::egui::{self, Id, Panel, RichText, Ui};

use super::{Action, Page, TbdSubtitlesApp, feature_views};
use crate::core::ui::MUTED_TEXT;

impl eframe::App for TbdSubtitlesApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.poll();
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
            .default_size(340.0)
            .show(ui, |ui| feature_views::queue_ui(ui, self, &mut actions));
        egui::CentralPanel::default().show(ui, |ui| match self.page {
            Page::Settings => feature_views::settings_ui(ui, self, &mut actions),
            Page::Jobs => {
                ui.heading(super::APP_NAME);
                ui.label(
                    RichText::new("Select a job in the queue to see its progress and report.")
                        .color(MUTED_TEXT),
                );
            }
        });
        actions
    }
}
