//! The overlay while files are dragged over the window: a blue wash with a card that says to
//! drop them to add videos.
//!
//! **Role:** show the owner that the window takes the files being dragged.
//!
//! **Position:** called by the application's frame after everything else, so it covers the
//! window; the files themselves arrive as dropped files and are queued by the application.
//!
//! **Signals and state:** none; reads whether files hover over the window.
//!
//! **Invariants:** nothing shows unless files hover.

use eframe::egui::{
    Align2, Area, Context, CornerRadius, FontFamily, FontId, Frame, Id, Margin, Order, RichText,
    Stroke, StrokeKind, vec2,
};

use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;

/// Cover the window while files hover over it.
pub(crate) fn drop_overlay_ui(ctx: &Context) {
    if ctx.input(|input| input.raw.hovered_files.is_empty()) {
        return;
    }
    let screen = ctx.content_rect();
    Area::new(Id::new("drop-overlay"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .interactable(false)
        .show(ctx, |ui| {
            let p = palette(ui);
            ui.painter().rect(
                screen,
                CornerRadius::same(10),
                p.accent_fill.gamma_multiply(0.12),
                Stroke::new(3.0, p.accent_fill),
                StrokeKind::Inside,
            );
        });
    Area::new(Id::new("drop-card"))
        .order(Order::Tooltip)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .interactable(false)
        .show(ctx, |ui| {
            let p = palette(ui);
            Frame::new()
                .fill(p.card)
                .corner_radius(CornerRadius::same(12))
                .inner_margin(Margin::symmetric(30, 22))
                .shadow(ui.visuals().popup_shadow)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.spacing_mut().item_spacing.y = 6.0;
                        ui.label(
                            RichText::new(icons::FILM_STRIP)
                                .font(icons::font(28.0))
                                .color(p.accent),
                        );
                        let semibold = FontFamily::Name(fonts::SEMIBOLD.into());
                        ui.label(
                            RichText::new("Drop to add videos")
                                .font(FontId::new(13.0, semibold))
                                .color(p.text),
                        );
                        ui.label(
                            RichText::new("A folder adds only its videos without subtitles.")
                                .size(12.0)
                                .color(p.text2),
                        );
                    });
                });
        });
}
