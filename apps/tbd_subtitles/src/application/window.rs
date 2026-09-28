//! One frame of the window: the desktop's colour scheme, the shortcuts, the toolbar across the
//! top, the sidebar on the left, the selected job on the right, then the drop overlay, the
//! toasts, the Settings window, and the actions they asked for.
//!
//! **Role:** run `poll`, draw the frame from the borrowed state, and apply the actions it
//! collected.
//!
//! **Position:** `eframe::App::ui` of `TbdSubtitlesApp`; lays out the panels and calls
//! `shortcuts`, `feature_views`, `settings_window`, the queue's drop overlay and the toasts.
//!
//! **Signals and state:** reads dropped files; asks for a frame each second while a job runs and
//! when the next toast is due to go.
//!
//! **Invariants:** `frame_ui` changes nothing; the toolbar is 52 px high and the sidebar 272 px
//! wide; the toasts, the overlay and the Settings window are drawn over the panes.

use std::time::{Duration, Instant};

use eframe::egui::{self, Frame, Id, Margin, Panel, Ui};

use super::{Action, TbdSubtitlesApp, feature_views, settings_window, shortcuts};
use crate::core::ui::palette::palette;
use crate::core::ui::theme;
use crate::core::ui::toast::toasts_ui;
use crate::job_queue::ui::drop_overlay_ui;

/// How often the window redraws while a job runs, so its clock and time left move.
const RUNNING_REDRAW: Duration = Duration::from_secs(1);
/// The toolbar's height and the sidebar's width.
const TOOLBAR_HEIGHT: f32 = 52.0;
const SIDEBAR_WIDTH: f32 = 272.0;

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
        let ctx = ui.ctx().clone();
        let dropped = ctx.input(|input| {
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
            ctx.request_repaint_after(RUNNING_REDRAW);
        }
        if let Some(next) = self.toasts.next_expiry() {
            ctx.request_repaint_after(next.saturating_duration_since(Instant::now()));
        }
        shortcuts::shortcuts(&ctx, self, &mut actions);
        let p = palette(ui);
        Panel::top(Id::new("toolbar"))
            .exact_size(TOOLBAR_HEIGHT)
            .frame(Frame::new().fill(p.toolbar).inner_margin(Margin {
                left: 16,
                right: 12,
                top: 0,
                bottom: 0,
            }))
            .show(ui, |ui| feature_views::toolbar_ui(ui, self, &mut actions));
        Panel::left(Id::new("sidebar"))
            .exact_size(SIDEBAR_WIDTH)
            .resizable(false)
            .frame(Frame::new().fill(p.sidebar).inner_margin(Margin {
                left: 8,
                right: 8,
                top: 8,
                bottom: 0,
            }))
            .show(ui, |ui| feature_views::sidebar_ui(ui, self, &mut actions));
        egui::CentralPanel::default()
            .frame(Frame::new().fill(p.window))
            .show(ui, |ui| feature_views::jobs_ui(ui, self, &mut actions));
        drop_overlay_ui(&ctx);
        if let Some(id) = toasts_ui(&ctx, self.toasts.shown()) {
            actions.push(Action::ToastButton(id));
        }
        let raise = actions.contains(&Action::ShowSettings(true));
        settings_window::settings_window_ui(&ctx, self, raise, &mut actions);
        actions
    }
}
