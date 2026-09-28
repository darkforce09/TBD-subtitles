//! The toasts, drawn at the bottom centre of the window above everything else.
//!
//! **Role:** draw each toast as a small card: its mark, its text and its button; an error's
//! border and text are red.
//!
//! **Position:** called by the application's frame with `core::toast::Toasts::shown`.
//!
//! **Signals and state:** none; returns the toast whose button was pressed.
//!
//! **Invariants:** the oldest toast sits at the bottom and newer ones stack above it.

use eframe::egui::{
    Align2, Area, Context, CornerRadius, Frame, Id, Label, Margin, Order, RichText, Stroke, Ui,
    vec2,
};

use crate::core::toast::{Toast, ToastId, ToastKind};
use crate::core::ui::button::{Button, ButtonSize};
use crate::core::ui::icons::{StatusIcon, status_icon};
use crate::core::ui::palette::palette;

/// The space between the window's bottom edge and the lowest toast.
const BOTTOM: f32 = 18.0;

/// Draw `toasts` at the bottom centre; the toast whose button was pressed, if one was.
pub(crate) fn toasts_ui<A>(ctx: &Context, toasts: &[Toast<A>]) -> Option<ToastId> {
    if toasts.is_empty() {
        return None;
    }
    let mut pressed = None;
    Area::new(Id::new("toasts"))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -BOTTOM))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            for toast in toasts.iter().rev() {
                if toast_ui(ui, toast) {
                    pressed = Some(toast.id);
                }
            }
        });
    pressed
}

/// One toast; whether its button was pressed.
fn toast_ui<A>(ui: &mut Ui, toast: &Toast<A>) -> bool {
    let p = palette(ui);
    let error = toast.kind == ToastKind::Error;
    let icon = match toast.kind {
        ToastKind::Success => StatusIcon::Done,
        ToastKind::Info => StatusIcon::Info,
        ToastKind::Working => StatusIcon::Working,
        ToastKind::Error => StatusIcon::Failed,
    };
    let mut pressed = false;
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(
            1.0,
            if error { p.bad_icon } else { p.line_strong },
        ))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::symmetric(12, 9))
        .shadow(ui.visuals().popup_shadow)
        .show(ui, |ui| {
            ui.set_min_width(236.0);
            ui.set_max_width(496.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                status_icon(ui, icon, 18.0, false);
                let text = RichText::new(&toast.text).color(if error { p.bad } else { p.text });
                ui.add(Label::new(text).wrap());
                if let Some((label, _)) = &toast.action {
                    pressed = Button::new(label)
                        .size(ButtonSize::Small)
                        .show(ui)
                        .clicked();
                }
            });
        });
    pressed
}
