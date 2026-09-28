//! A switch as the mockup draws it: a 32 by 19 px rounded track, grey when off and blue when on,
//! with a white knob that sits right while on.
//!
//! **Role:** paint the switch, say when it was clicked, and name it for accessibility with
//! whether it is on.
//!
//! **Position:** used by the line editor's four flags.
//!
//! **Signals and state:** none; the caller holds whether it is on.
//!
//! **Invariants:** the knob is 15 px, 2 px inside the track, left while off and right while on.

use eframe::egui::{
    Color32, CornerRadius, Rect, Response, Sense, Shadow, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::core::ui::palette::palette;

/// The track's size, and the knob's.
const WIDTH: f32 = 32.0;
const HEIGHT: f32 = 19.0;
const KNOB: f32 = 15.0;

/// Draw a switch that is `on`, named `name`; `clicked()` on the answer means turn it over.
pub(crate) fn switch(ui: &mut Ui, on: bool, name: &str) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(vec2(WIDTH, HEIGHT), Sense::click());
    response.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, true, on, name));
    let track = if on { p.accent_fill } else { p.line_strong };
    let track = if response.hovered() {
        track.blend(p.hover)
    } else {
        track
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same(10), track);
    let inset = (HEIGHT - KNOB) / 2.0;
    let left = if on {
        rect.right() - inset - KNOB
    } else {
        rect.left() + inset
    };
    let knob = Rect::from_min_size(pos2(left, rect.top() + inset), vec2(KNOB, KNOB));
    let radius = CornerRadius::same(8);
    let shadow = Shadow {
        offset: [0, 1],
        blur: 2,
        spread: 0,
        color: Color32::from_black_alpha(64),
    };
    ui.painter().add(shadow.as_shape(knob, radius));
    ui.painter().rect_filled(knob, radius, Color32::WHITE);
    response
}
