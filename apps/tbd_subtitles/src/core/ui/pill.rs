//! Pills and count badges as the mockup draws them: a short state in its colour on a pale tint of
//! it, and the orange count of lines still to check at the end of a finished row.
//!
//! **Role:** paint the verdict pill beside a card's title and the count badge of a sidebar row,
//! from the palette.
//!
//! **Position:** used by the job report's file card and the queue's sidebar rows.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a pill is 20 px high with radius 10, its text in 11.5 px semibold; a badge is
//! 18 px high and at least 22 px wide, its count in 11 px bold, white on a selected row.

use eframe::egui::{
    Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, Sense, Ui, WidgetInfo,
    WidgetType, pos2, vec2,
};

use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;

/// A pill's height, the space inside its ends, and between its mark and its text.
const PILL_HEIGHT: f32 = 20.0;
const PILL_PADDING: f32 = 8.0;
const PILL_GAP: f32 = 4.0;
/// A badge's height, its least width, and the space inside its ends.
const BADGE_HEIGHT: f32 = 18.0;
const BADGE_MIN_WIDTH: f32 = 22.0;
const BADGE_PADDING: f32 = 6.0;

/// A pill's colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    /// Green: in order.
    Good,
    /// Orange: worth a look.
    Warn,
}

/// Draw a pill of `tone` with `glyph` before `text`, named by its text for accessibility.
pub(crate) fn pill(ui: &mut Ui, tone: Tone, glyph: &str, text: &str) -> Response {
    let p = palette(ui);
    let (colour, tint) = match tone {
        Tone::Good => (p.good, p.good_tint),
        Tone::Warn => (p.warn, p.warn_tint),
    };
    let semibold = FontId::new(11.5, FontFamily::Name(fonts::SEMIBOLD.into()));
    let label = ui
        .painter()
        .layout_no_wrap(text.to_string(), semibold, colour);
    let icon = ui
        .painter()
        .layout_no_wrap(glyph.to_string(), icons::font(14.0), colour);
    let width = 2.0 * PILL_PADDING + icon.size().x + PILL_GAP + label.size().x;
    let (rect, response) = ui.allocate_exact_size(vec2(width, PILL_HEIGHT), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, true, text));
    ui.painter().rect_filled(rect, CornerRadius::same(10), tint);
    let icon_left = rect.left() + PILL_PADDING;
    let label_left = icon_left + icon.size().x + PILL_GAP;
    let centre = rect.center().y;
    ui.painter()
        .galley(pos2(icon_left, centre - icon.size().y / 2.0), icon, colour);
    ui.painter().galley(
        pos2(label_left, centre - label.size().y / 2.0),
        label,
        colour,
    );
    response
}

/// Paint the badge counting `count` with the middle of its right edge at `right`: orange on its
/// tint, or white on a translucent white when `on_accent` (a selected row). The space it took.
pub(crate) fn paint_badge(ui: &Ui, right: Pos2, count: usize, on_accent: bool) -> Rect {
    let p = palette(ui);
    let (text, fill) = if on_accent {
        (Color32::WHITE, Color32::from_white_alpha(64))
    } else {
        (p.warn, p.warn_tint)
    };
    let bold = FontId::new(11.0, FontFamily::Name(fonts::BOLD.into()));
    let galley = ui.painter().layout_no_wrap(count.to_string(), bold, text);
    let width = (galley.size().x + 2.0 * BADGE_PADDING).max(BADGE_MIN_WIDTH);
    let rect = Rect::from_min_max(
        pos2(right.x - width, right.y - BADGE_HEIGHT / 2.0),
        pos2(right.x, right.y + BADGE_HEIGHT / 2.0),
    );
    ui.painter().rect_filled(rect, CornerRadius::same(9), fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2.0, galley, text);
    rect
}
