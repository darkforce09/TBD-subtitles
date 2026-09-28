//! The pieces of the Settings window's forms, as the mockup draws them: a row of a 170 px label,
//! right-aligned, beside its controls; the help and error lines under a control; the dividing
//! line; a path in its well beside its buttons; a list to choose from; and a stepper whose typed
//! number is sent once the owner is done.
//!
//! **Role:** lay out and paint the form's parts from the palette, and say what the owner changed.
//!
//! **Position:** used by the General and Engines tabs of `settings_window`, and for a folder by the
//! This Computer tab.
//!
//! **Signals and state:** the number being typed in a stepper lives in egui's memory while
//! the field has the focus, and goes once it is sent.
//!
//! **Invariants:** a stepper's typed number is sent on Enter, when the focus leaves or when
//! the window closes, never while it is typed; a typed number that is not finite or not in its
//! range is dropped; a stepper's arrows send at once and keep the value in its range; a path
//! stays on one line, the home as `~`, cut in the middle, whole on hover.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use eframe::egui::{
    Align, Align2, Color32, ComboBox, CornerRadius, FontId, Frame, Id, Label, Layout, Margin,
    Response, RichText, Sense, Shape, Stroke, StrokeKind, TextEdit, TextStyle, Ui, WidgetInfo,
    WidgetType, vec2,
};

use crate::core::ui::icons::{self, StatusIcon, status_icon};
use crate::core::ui::palette::palette;
use crate::settings::models::page::{Field, SettingsPage};

/// The label column's width, the space beside it, and the space between rows.
const LABEL_WIDTH: f32 = 170.0;
const COLUMN_GAP: f32 = 16.0;
pub(super) const ROW_GAP: f32 = 14.0;
/// A control's height.
const CONTROL_HEIGHT: f32 = 28.0;

/// A form row: `label` right-aligned in its column, then the controls `add` draws, 5 px apart.
pub(super) fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = COLUMN_GAP;
        let (rect, _) = ui.allocate_exact_size(vec2(LABEL_WIDTH, CONTROL_HEIGHT), Sense::hover());
        let p = palette(ui);
        ui.painter().text(
            rect.right_center(),
            Align2::RIGHT_CENTER,
            label,
            FontId::proportional(13.0),
            p.text,
        );
        ui.vertical(|ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 5.0;
            add(ui);
        });
    });
}

/// A 1 px line across the form.
pub(super) fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().hline(
        rect.x_range(),
        rect.center().y,
        Stroke::new(1.0, palette(ui).line),
    );
}

/// A help line under a control: 11.5 px grey, wrapped.
pub(super) fn help(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    ui.add(Label::new(RichText::new(text).size(11.5).color(p.text2)).wrap());
}

/// A red line with a cross: why something was not done.
pub(super) fn error(ui: &mut Ui, text: &str) {
    let p = palette(ui);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        status_icon(ui, StatusIcon::Failed, 14.0, false);
        ui.add(Label::new(RichText::new(text).size(12.0).color(p.bad)).wrap());
    });
}

/// The error under `field`, when the last edit of it was refused.
pub(super) fn field_error(ui: &mut Ui, page: &SettingsPage, field: Field) {
    if let Some(refused) = page.error.as_ref().filter(|e| e.field == field) {
        error(ui, &refused.message);
    }
}

/// A path in the monospace well on one line, as wide as the buttons beside it leave: the home
/// folder as `~`, cut in the middle when it is still too long, the whole path on hover; `buttons`
/// draws them from the right, the last first.
pub(super) fn path_line(ui: &mut Ui, path: &Path, buttons: impl FnOnce(&mut Ui)) {
    let size = vec2(ui.available_width(), CONTROL_HEIGHT);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        buttons(ui);
        let rest = vec2(ui.available_width(), CONTROL_HEIGHT);
        ui.allocate_ui_with_layout(rest, Layout::left_to_right(Align::Center), |ui| {
            let p = palette(ui);
            Frame::new()
                .fill(p.well)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(9, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let font = TextStyle::Monospace.resolve(ui.style());
                    let shown = fit_middle(ui, &tilde(path), &font, ui.available_width());
                    ui.add(Label::new(RichText::new(shown).font(font).color(p.text2)).truncate())
                        .on_hover_text(path.display().to_string());
                });
        });
    });
}

/// `path` with the home folder written `~`.
pub(super) fn tilde(path: &Path) -> String {
    tilde_under(path, std::env::var_os("HOME").map(PathBuf::from).as_deref())
}

/// `path` with `home` written `~`, when it is under it.
pub(super) fn tilde_under(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_string(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

/// `text` in `font`, cut in the middle with "…" so it fits `width`; whole when it fits.
pub(super) fn fit_middle(ui: &Ui, text: &str, font: &FontId, width: f32) -> String {
    let fits = |shown: &str| {
        ui.painter()
            .layout_no_wrap(shown.to_string(), font.clone(), Color32::WHITE)
            .size()
            .x
            <= width
    };
    if fits(text) {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let cut = |kept: usize| {
        let head = kept / 2;
        let tail = kept - head;
        let mut shown: String = chars[..head].iter().collect();
        shown.push('…');
        shown.extend(&chars[chars.len() - tail..]);
        shown
    };
    let (mut low, mut high) = (0, chars.len());
    while low < high {
        let middle = (low + high).div_ceil(2);
        if fits(&cut(middle)) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    cut(low)
}

/// A list of `options` with `current` shown, `width` wide; the value chosen, when it differs.
pub(super) fn choice<T: Clone + PartialEq>(
    ui: &mut Ui,
    id: &str,
    current: &T,
    options: &[(T, String)],
    width: f32,
) -> Option<T> {
    let mut chosen = current.clone();
    let shown = options
        .iter()
        .find(|(value, _)| value == current)
        .map_or("", |(_, label)| label.as_str());
    ComboBox::from_id_salt(id)
        .selected_text(shown)
        .width(width)
        .height(CONTROL_HEIGHT * 8.0)
        .show_ui(ui, |ui| {
            for (value, label) in options {
                ui.selectable_value(&mut chosen, value.clone(), label.as_str());
            }
        });
    (chosen != *current).then_some(chosen)
}

/// Draw a field with `add` over the text being typed (kept in egui's memory under `id` while the
/// field has the focus, else `value`); the text, and whether the owner is done with it: the field
/// lost the focus, or the window is `closing`, or the focus went elsewhere with text still kept.
/// The kept text goes once it is done.
fn typed(
    ui: &mut Ui,
    id: Id,
    value: &str,
    closing: bool,
    add: impl FnOnce(&mut Ui, &mut String) -> Response,
) -> (String, bool) {
    let key = id.with("typed");
    let focused = ui.memory(|memory| memory.has_focus(id));
    let kept = ui.data(|data| data.get_temp::<String>(key));
    let mut text = kept
        .clone()
        .filter(|_| focused)
        .unwrap_or_else(|| value.to_string());
    let response = add(ui, &mut text);
    let forget = |ui: &mut Ui| ui.data_mut(|data| data.remove::<String>(key));
    if response.lost_focus() || (closing && response.has_focus()) {
        forget(ui);
        if closing {
            response.surrender_focus();
        }
        return (text, true);
    }
    if !response.has_focus() {
        return match kept {
            Some(kept) => {
                forget(ui);
                (kept, true)
            }
            None => (text, false),
        };
    }
    ui.data_mut(|data| data.insert_temp(key, text.clone()));
    (text, false)
}

/// The number typed into a stepper, when it is one, finite and within `range`.
pub(super) fn typed_number(text: &str, range: &RangeInclusive<f64>) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|number| number.is_finite() && range.contains(number))
}

/// A stepper named `name`: a down arrow, the number, an up arrow, in `range`, one a step. The new
/// value when an arrow is pressed, or when a typed number is done (the window `closing` counts)
/// and is finite and within the range; anything else typed is dropped.
pub(super) fn stepper(
    ui: &mut Ui,
    name: &str,
    value: f64,
    range: RangeInclusive<f64>,
    closing: bool,
) -> Option<f64> {
    let p = palette(ui);
    let (min, max) = (*range.start(), *range.end());
    let background = ui.painter().add(Shape::Noop);
    let id = ui.id().with(("stepper", name));
    let mut changed = None;
    let rect = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let left = CornerRadius {
                nw: 6,
                sw: 6,
                ne: 0,
                se: 0,
            };
            if arrow(ui, icons::CARET_DOWN, "Less", value > min, left) {
                changed = Some((value - 1.0).clamp(min, max));
            }
            let (text, done) = typed(ui, id, &number(value), closing, |ui, text| {
                ui.add(
                    TextEdit::singleline(text)
                        .id(id)
                        .frame(Frame::NONE)
                        .horizontal_align(Align::Center)
                        .desired_width(48.0)
                        .min_size(vec2(48.0, CONTROL_HEIGHT))
                        .vertical_align(Align::Center)
                        .margin(Margin::ZERO),
                )
            });
            if done
                && let Some(typed) = typed_number(&text, &range)
                && typed != value
            {
                changed = Some(typed);
            }
            let right = CornerRadius {
                nw: 0,
                sw: 0,
                ne: 6,
                se: 6,
            };
            if arrow(ui, icons::CARET_UP, "More", value < max, right) {
                changed = Some((value + 1.0).clamp(min, max));
            }
        })
        .response
        .rect;
    let radius = CornerRadius::same(6);
    ui.painter()
        .set(background, Shape::rect_filled(rect, radius, p.control));
    ui.painter().rect_stroke(
        rect,
        radius,
        Stroke::new(1.0, p.control_border),
        StrokeKind::Inside,
    );
    changed
}

/// One of a stepper's arrows, 24 by 28 px on the grey of a well, named `name`; whether it was
/// pressed.
fn arrow(ui: &mut Ui, glyph: &str, name: &str, enabled: bool, round: CornerRadius) -> bool {
    let p = palette(ui);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(24.0, CONTROL_HEIGHT), sense);
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, name));
    let fill = if enabled && response.hovered() {
        p.seg_bg.blend(p.hover)
    } else {
        p.seg_bg
    };
    ui.painter().rect_filled(rect, round, fill);
    let colour = if enabled {
        p.text
    } else {
        p.text.gamma_multiply(0.45)
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        icons::font(14.0),
        colour,
    );
    response.clicked()
}

/// A number as the stepper shows it: whole numbers without a point.
fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
#[path = "tests/form.rs"]
mod tests;
