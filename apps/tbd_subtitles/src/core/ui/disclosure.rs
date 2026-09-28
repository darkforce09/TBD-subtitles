//! A disclosure as the mockup draws it: a full-width row with a chevron that points down while
//! open, its label in semibold, and a quiet note on the right.
//!
//! **Role:** paint the row, turn a click into open or closed, and name it for accessibility by
//! the label it shows.
//!
//! **Position:** used by cards whose details fold away, such as the stages of a running job.
//!
//! **Signals and state:** whether it is open lives in egui's memory under the caller's id, so it
//! survives frames and is forgotten when the window closes.
//!
//! **Invariants:** a disclosure starts closed; the row is 12 px above and below its text and 18 px
//! from the card's edges.

use eframe::egui::{Align2, FontFamily, FontId, Id, Sense, Ui, WidgetInfo, WidgetType, pos2, vec2};

use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;

/// The row's height, and the space from the card's edges to its content.
const HEIGHT: f32 = 42.0;
const SIDE: f32 = 18.0;
/// The space between the chevron and the label.
const GAP: f32 = 8.0;

/// Draw the disclosure kept under `id`, showing `closed` or `open` as its label and `aside` on
/// the right; whether it is open after this frame's click.
pub(crate) fn disclosure(ui: &mut Ui, id: Id, (closed, open): (&str, &str), aside: &str) -> bool {
    let p = palette(ui);
    let mut is_open = ui.data(|data| data.get_temp::<bool>(id)).unwrap_or(false);
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::click());
    if response.clicked() {
        is_open = !is_open;
        ui.data_mut(|data| data.insert_temp(id, is_open));
    }
    let label = if is_open { open } else { closed };
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    let chevron = if is_open {
        icons::CARET_DOWN
    } else {
        icons::CARET_RIGHT
    };
    let centre = rect.center().y;
    let chevron = ui.painter().text(
        pos2(rect.left() + SIDE, centre),
        Align2::LEFT_CENTER,
        chevron,
        icons::font(14.0),
        p.text2,
    );
    ui.painter().text(
        pos2(chevron.right() + GAP, centre),
        Align2::LEFT_CENTER,
        label,
        FontId::new(13.0, FontFamily::Name(fonts::SEMIBOLD.into())),
        p.text,
    );
    ui.painter().text(
        pos2(rect.right() - SIDE, centre),
        Align2::RIGHT_CENTER,
        aside,
        FontId::proportional(13.0),
        p.text2,
    );
    is_open
}
