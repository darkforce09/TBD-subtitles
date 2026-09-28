//! A segmented control as the mockup draws it: a grey rounded well of segments, the chosen one
//! raised in a lighter fill with a small shadow, a segment's count or check after its label.
//!
//! **Role:** paint the segments, name each for accessibility with whether it is chosen, and say
//! which one was clicked.
//!
//! **Position:** used by the views that switch between tabs, such as the selected job's Overview
//! and Check Lines, and across the top of the line list for its three lists.
//!
//! **Signals and state:** none; the caller holds which segment is chosen.
//!
//! **Invariants:** a click on the chosen segment asks for nothing; every segment is 24 px high
//! inside a 2 px well; a segment is named by its label alone, its tally aside.

use std::sync::Arc;

use eframe::egui::{
    Color32, CornerRadius, FontFamily, FontId, Galley, Rect, Sense, Shadow, Ui, WidgetInfo,
    WidgetType, pos2, vec2,
};

use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;

/// A segment's height, its side padding, and the well around the segments.
const HEIGHT: f32 = 24.0;
const PADDING: f32 = 12.0;
const WELL: f32 = 2.0;
/// The space between a segment's label and its tally.
const TALLY_GAP: f32 = 6.0;

/// What a segment shows after its label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tally {
    /// A count in small grey figures.
    Count(usize),
    /// A green check: none left.
    Done,
}

/// Draw one segment per `(value, label, tally)` with `current` chosen, each as wide as its label;
/// the value clicked, when it is not the one chosen.
pub(crate) fn segmented<T: Copy + PartialEq>(
    ui: &mut Ui,
    current: T,
    segments: &[(T, &str, Option<Tally>)],
) -> Option<T> {
    draw(ui, current, segments, false)
}

/// As [`segmented`], across the width given, the segments equally wide.
pub(crate) fn segmented_across<T: Copy + PartialEq>(
    ui: &mut Ui,
    current: T,
    segments: &[(T, &str, Option<Tally>)],
) -> Option<T> {
    draw(ui, current, segments, true)
}

fn draw<T: Copy + PartialEq>(
    ui: &mut Ui,
    current: T,
    segments: &[(T, &str, Option<Tally>)],
    across: bool,
) -> Option<T> {
    let p = palette(ui);
    let font = FontId::proportional(12.5);
    let laid: Vec<(Arc<Galley>, Option<Arc<Galley>>)> = segments
        .iter()
        .map(|(_, label, tally)| {
            let label = ui
                .painter()
                .layout_no_wrap((*label).to_string(), font.clone(), p.text);
            let tally = tally.map(|tally| tally_galley(ui, tally));
            (label, tally)
        })
        .collect();
    let gaps = WELL * segments.len().saturating_sub(1) as f32;
    let widths: Vec<f32> = if across {
        let each = (ui.available_width() - gaps - 2.0 * WELL) / segments.len().max(1) as f32;
        vec![each.max(0.0); segments.len()]
    } else {
        laid.iter()
            .map(|(label, tally)| {
                let tally = tally
                    .as_ref()
                    .map_or(0.0, |tally| TALLY_GAP + tally.size().x);
                label.size().x + tally + 2.0 * PADDING
            })
            .collect()
    };
    let size = vec2(
        widths.iter().sum::<f32>() + gaps + 2.0 * WELL,
        HEIGHT + 2.0 * WELL,
    );
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter()
        .rect_filled(rect, CornerRadius::same(8), p.seg_bg);
    let mut clicked = None;
    let mut x = rect.left() + WELL;
    for (i, (((value, label, _), (galley, tally)), width)) in
        segments.iter().zip(laid).zip(widths).enumerate()
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
        let tally_width = tally
            .as_ref()
            .map_or(0.0, |tally| TALLY_GAP + tally.size().x);
        let left = segment.center().x - (galley.size().x + tally_width) / 2.0;
        let centre = segment.center().y;
        let label_right = left + galley.size().x;
        ui.painter()
            .galley(pos2(left, centre - galley.size().y / 2.0), galley, p.text);
        if let Some(tally) = tally {
            let at = pos2(label_right + TALLY_GAP, centre - tally.size().y / 2.0);
            ui.painter().galley(at, tally, p.text2);
        }
        if answer.clicked() && !chosen {
            clicked = Some(*value);
        }
    }
    clicked
}

/// A count in 11 px semibold grey, or a 14 px green check.
fn tally_galley(ui: &Ui, tally: Tally) -> Arc<Galley> {
    let p = palette(ui);
    match tally {
        Tally::Count(count) => ui.painter().layout_no_wrap(
            count.to_string(),
            FontId::new(11.0, FontFamily::Name(fonts::SEMIBOLD.into())),
            p.text2,
        ),
        Tally::Done => {
            ui.painter()
                .layout_no_wrap(icons::CHECK.to_string(), icons::font(14.0), p.good)
        }
    }
}
