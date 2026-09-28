//! A segmented control as the mockup draws it: a grey rounded well of segments, the chosen one
//! raised in a lighter fill with a small shadow.
//!
//! **Role:** paint the segments, name each for accessibility with whether it is chosen, and say
//! which one was clicked.
//!
//! **Position:** used by the views that switch between tabs, such as the selected job's Overview
//! and Check Lines.
//!
//! **Signals and state:** none; the caller holds which segment is chosen.
//!
//! **Invariants:** a click on the chosen segment asks for nothing; every segment is 24 px high
//! inside a 2 px well.

use eframe::egui::{
    Color32, CornerRadius, FontId, Rect, Sense, Shadow, Ui, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::core::ui::palette::palette;

/// A segment's height, its side padding, and the well around the segments.
const HEIGHT: f32 = 24.0;
const PADDING: f32 = 12.0;
const WELL: f32 = 2.0;

/// Draw one segment per `(value, label)` with `current` chosen; the value clicked, when it is
/// not the one chosen.
pub(crate) fn segmented<T: Copy + PartialEq>(
    ui: &mut Ui,
    current: T,
    segments: &[(T, &str)],
) -> Option<T> {
    let p = palette(ui);
    let font = FontId::proportional(12.5);
    let galleys: Vec<_> = segments
        .iter()
        .map(|(_, label)| {
            ui.painter()
                .layout_no_wrap((*label).to_string(), font.clone(), p.text)
        })
        .collect();
    let widths: Vec<f32> = galleys
        .iter()
        .map(|galley| galley.size().x + 2.0 * PADDING)
        .collect();
    let gaps = WELL * segments.len().saturating_sub(1) as f32;
    let size = vec2(
        widths.iter().sum::<f32>() + gaps + 2.0 * WELL,
        HEIGHT + 2.0 * WELL,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), p.seg_bg);
    let mut clicked = None;
    let mut x = rect.left() + WELL;
    for (i, (((value, label), galley), width)) in
        segments.iter().zip(galleys).zip(widths).enumerate()
    {
        let segment = Rect::from_min_size(pos2(x, rect.top() + WELL), vec2(width, HEIGHT));
        x += width + WELL;
        let chosen = *value == current;
        let answer = ui.interact(segment, response.id.with(i), Sense::click());
        answer.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, chosen, *label));
        let radius = CornerRadius::same(6);
        if chosen {
            let shadow = Shadow {
                offset: [0, 1],
                blur: 2,
                spread: 0,
                color: Color32::from_black_alpha(31),
            };
            ui.painter().add(shadow.as_shape(segment, radius));
            ui.painter().rect_filled(segment, radius, p.seg_on);
        } else if answer.hovered() {
            ui.painter().rect_filled(segment, radius, p.hover);
        }
        let at = segment.center() - galley.size() / 2.0;
        ui.painter().galley(at, galley, p.text);
        if answer.clicked() && !chosen {
            clicked = Some(*value);
        }
    }
    clicked
}
