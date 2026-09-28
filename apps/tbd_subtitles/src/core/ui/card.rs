//! Cards as the mockup draws them: a raised panel with a border, its head of a mark, a title and
//! lines of text, and the recessed box that holds a path or a raw message.
//!
//! **Role:** paint the card frame, the card head and the recessed box from the palette, so every
//! card in the window looks alike.
//!
//! **Position:** used by the features' views of the selected job; draws marks with
//! `core::ui::icons`.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a card fills the width it is given; a padded card keeps 16 px above and below
//! its contents, 18 px beside them and 12 px between them.

use eframe::egui::{
    Color32, CornerRadius, Frame, Label, Margin, RichText, Shadow, Stroke, TextStyle, Ui,
};

use crate::core::ui::icons::{StatusIcon, status_icon};
use crate::core::ui::palette::palette;

/// The corner radius of cards.
const RADIUS: u8 = 10;
/// The space between a padded card's parts, and between a card head's mark and its text.
const GAP: f32 = 12.0;

/// Draw `add` on a card as wide as the space given: the card fill, a 1 px border, radius 10 and a
/// faint shadow; `padded` keeps 16 px above and below the contents and 18 px beside them.
pub(crate) fn card<R>(ui: &mut Ui, padded: bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = palette(ui);
    let margin = if padded {
        Margin::symmetric(18, 16)
    } else {
        Margin::ZERO
    };
    Frame::new()
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.line))
        .corner_radius(CornerRadius::same(RADIUS))
        .shadow(Shadow {
            offset: [0, 1],
            blur: 8,
            spread: 0,
            color: p.card_shadow,
        })
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = if padded { GAP } else { 0.0 };
            add(ui)
        })
        .inner
}

/// A card's head: a 28 px `mark`, then `title` as a headline over the lines `body` draws, 3 px
/// apart.
pub(crate) fn card_head(ui: &mut Ui, mark: StatusIcon, title: &str, body: impl FnOnce(&mut Ui)) {
    let p = palette(ui);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        status_icon(ui, mark, 28.0, false);
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 3.0;
            ui.label(
                RichText::new(title)
                    .text_style(TextStyle::Heading)
                    .color(p.text),
            );
            body(ui);
        });
    });
}

/// A line of a card's text in `colour`, wrapped to the card's width.
pub(crate) fn card_text(ui: &mut Ui, text: &str, colour: Color32) {
    ui.add(Label::new(RichText::new(text).color(colour)).wrap());
}

/// `text` in the monospace font on the recessed well, radius 6: one line cut with an ellipsis, or
/// wrapped when `wrap`.
pub(crate) fn well_text(ui: &mut Ui, text: &str, wrap: bool) {
    let p = palette(ui);
    Frame::new()
        .fill(p.well)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::symmetric(9, 6))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let label = Label::new(RichText::new(text).monospace().color(p.text2));
            ui.add(if wrap { label.wrap() } else { label.truncate() });
        });
}
