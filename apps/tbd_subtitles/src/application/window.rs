//! One frame of the window: the desktop's colour scheme, the shortcuts, the toolbar across the
//! top, the models banner under it, the sidebar on the left, the selected job on the right, then
//! the drop overlay, the toasts, the Settings window, the log window, and the actions they asked
//! for.
//!
//! **Role:** in `logic`, note egui's clock and whether the window is away, run `poll`, and
//! minimize the window, bring it forward or ask the desktop for its attention when asked; in
//! `ui`, draw the frame from the borrowed state and apply the actions it collected.
//!
//! **Position:** `eframe::App::logic` and `eframe::App::ui` of `TbdSubtitlesApp`; lays out the
//! panels and calls `shortcuts`, `feature_views`, `settings_window`, `log_window`, the queue's
//! drop overlay and the toasts.
//!
//! **Signals and state:** reads dropped files and whether the window is focused or minimized;
//! asks for a frame each second while a job or a Fix It run runs, when the next toast is due to
//! go, and when the banner that says every model is on disk goes.
//!
//! **Invariants:** `logic` runs while the window is minimized too, when eframe draws nothing, so
//! the queue and the threads' answers never wait for the window to show; `frame_ui` changes
//! nothing; the toolbar is 52 px high and the sidebar 272 px wide; the banner spans the window
//! under the toolbar while it shows; the toasts, the overlay and the Settings and log windows are
//! drawn over the panes.

use std::time::{Duration, Instant};

use eframe::egui::{
    self, Context, Frame, Id, Margin, Panel, Ui, UserAttentionType, ViewportCommand,
};

use super::{Action, TbdSubtitlesApp, feature_views, log_window, settings_window, shortcuts};
use crate::core::ui::palette::palette;
use crate::core::ui::theme;
use crate::core::ui::toast::toasts_ui;
use crate::job_queue::ui::drop_overlay_ui;
use crate::settings::events::SettingsEvent;
use crate::settings::services::model_list::Banner;

/// How often the window redraws while a job or a Fix It run runs, so its clock and time left
/// move and Fix It's note says whether it waits for a free call.
const RUNNING_REDRAW: Duration = Duration::from_secs(1);
/// The toolbar's height and the sidebar's width.
const TOOLBAR_HEIGHT: f32 = 52.0;
const SIDEBAR_WIDTH: f32 = 272.0;

/// What the window saw of itself at the last frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Presence {
    /// egui's time, in seconds.
    pub(super) time: f64,
    /// Whether the desktop reports the window unfocused or minimized; unknown is not away.
    pub(super) away: bool,
}

impl Presence {
    fn read(ctx: &Context) -> Presence {
        ctx.input(|input| {
            let viewport = input.viewport();
            Presence {
                time: input.time,
                away: viewport.focused == Some(false) || viewport.minimized == Some(true),
            }
        })
    }
}

impl eframe::App for TbdSubtitlesApp {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.presence = Presence::read(ctx);
        self.poll();
        if std::mem::take(&mut self.minimize_once) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        if std::mem::take(&mut self.raise) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
            self.attention = true;
        }
        if std::mem::take(&mut self.attention) {
            let informational = UserAttentionType::Informational;
            ctx.send_viewport_cmd(ViewportCommand::RequestUserAttention(informational));
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        theme::follow(ui.ctx(), self.scheme);
        let actions = self.frame_ui(ui);
        self.apply(actions);
        // An action that asks for the desktop's attention is carried out by the next `logic`.
        if self.attention || self.raise {
            ui.ctx().request_repaint();
        }
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
        // Fix It's note says when a run waits for a free call, which no thread announces.
        if self.queue.running_job().is_some() || !self.pending.fixes.is_empty() {
            ctx.request_repaint_after(RUNNING_REDRAW);
        }
        if let Some(next) = self.toasts.next_expiry() {
            ctx.request_repaint_after(next.saturating_duration_since(Instant::now()));
        }
        if let Some(deadline) = self.pending.next_open_deadline() {
            ctx.request_repaint_after(deadline.saturating_duration_since(Instant::now()));
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
        if let Some(banner) = feature_views::models_banner(self) {
            if let Banner::AllOnDisk { until } = &banner {
                ctx.request_repaint_after(until.saturating_duration_since(Instant::now()));
            }
            Panel::top(Id::new("models-banner"))
                .frame(Frame::new())
                .show(ui, |ui| {
                    feature_views::models_banner_ui(ui, &banner, &mut actions)
                });
        }
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
        let raise = actions.iter().any(|action| {
            matches!(
                action,
                Action::ShowSettings(true) | Action::Settings(SettingsEvent::Open(_))
            )
        });
        settings_window::settings_window_ui(&ctx, self, raise, &mut actions);
        let raise_log = actions.contains(&Action::ShowLog(true));
        log_window::log_window_ui(&ctx, self, raise_log, &mut actions);
        actions
    }
}
