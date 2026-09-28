//! Buttons as the mockup draws them: a bordered button with an optional icon, the blue primary
//! button, three heights, and a borderless icon button.
//!
//! **Role:** paint a button from the palette, with its hover, pressed and disabled looks, and
//! name it for accessibility.
//!
//! **Position:** used by the features' toolbars, cards and menus and by `core::ui::toast`.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a disabled button senses no click and is drawn at 45 % opacity; its label is
//! the name assistive tools and the tests find it by.

use eframe::egui::{
    Align2, Color32, CornerRadius, FontId, Response, Sense, Stroke, StrokeKind, Ui, Vec2,
    WidgetInfo, WidgetType, pos2, vec2,
};

use crate::core::ui::icons;
use crate::core::ui::palette::palette;

/// The corner radius of every button.
const RADIUS: u8 = 6;
/// The space between a button's icon and its label.
const ICON_GAP: f32 = 6.0;

/// A button's height: 24, 28 or 32 px.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ButtonSize {
    Small,
    Regular,
    Large,
}

/// A bordered button with an optional icon; `primary` draws it blue.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Button<'a> {
    label: &'a str,
    icon: Option<&'a str>,
    primary: bool,
    size: ButtonSize,
    enabled: bool,
    min_width: f32,
}

impl<'a> Button<'a> {
    pub(crate) fn new(label: &'a str) -> Button<'a> {
        Button {
            label,
            icon: None,
            primary: false,
            size: ButtonSize::Regular,
            enabled: true,
            min_width: 0.0,
        }
    }

    /// A Phosphor glyph before the label.
    pub(crate) fn icon(self, glyph: &'a str) -> Button<'a> {
        Button {
            icon: Some(glyph),
            ..self
        }
    }

    /// The blue button: the one thing to do next.
    pub(crate) fn primary(self, primary: bool) -> Button<'a> {
        Button { primary, ..self }
    }

    pub(crate) fn size(self, size: ButtonSize) -> Button<'a> {
        Button { size, ..self }
    }

    pub(crate) fn enabled(self, enabled: bool) -> Button<'a> {
        Button { enabled, ..self }
    }

    /// At least this wide, the content centred.
    pub(crate) fn min_width(self, min_width: f32) -> Button<'a> {
        Button { min_width, ..self }
    }

    /// Draw the button; `clicked()` on the answer says it was pressed.
    pub(crate) fn show(self, ui: &mut Ui) -> Response {
        let p = palette(ui);
        let (height, padding, text_size) = match self.size {
            ButtonSize::Small => (24.0, 9.0, 12.0),
            ButtonSize::Regular => (28.0, 12.0, 13.0),
            ButtonSize::Large => (32.0, 16.0, 13.0),
        };
        let text = if self.primary { Color32::WHITE } else { p.text };
        let label = ui.painter().layout_no_wrap(
            self.label.to_string(),
            FontId::proportional(text_size),
            text,
        );
        let icon = self.icon.map(|glyph| {
            ui.painter()
                .layout_no_wrap(glyph.to_string(), icons::font(14.0), text)
        });
        let icon_width = icon.as_ref().map_or(0.0, |icon| icon.size().x + ICON_GAP);
        let content = icon_width + label.size().x;
        let width = (content + 2.0 * padding).max(self.min_width);
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);
        let (enabled, name) = (self.enabled, self.label.to_string());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &name));
        let pressed = response.is_pointer_button_down_on();
        let (base, border) = if self.primary {
            (p.accent_fill, p.accent_fill)
        } else {
            (p.control, p.control_border)
        };
        let overlay = if !self.enabled {
            None
        } else if pressed {
            Some(if self.primary {
                Color32::from_white_alpha(36)
            } else {
                p.press
            })
        } else if response.hovered() {
            Some(if self.primary {
                Color32::from_white_alpha(20)
            } else {
                p.hover
            })
        } else {
            None
        };
        let fill = overlay.map_or(base, |overlay| base.blend(overlay));
        let fade = |colour: Color32| {
            if self.enabled {
                colour
            } else {
                colour.gamma_multiply(0.45)
            }
        };
        ui.painter().rect(
            rect,
            CornerRadius::same(RADIUS),
            fade(fill),
            Stroke::new(1.0, fade(border)),
            StrokeKind::Inside,
        );
        let mut x = rect.center().x - content / 2.0;
        if let Some(icon) = icon {
            let size = icon.size();
            let at = pos2(x, rect.center().y - size.y / 2.0);
            x += size.x + ICON_GAP;
            ui.painter()
                .galley_with_override_text_color(at, icon, fade(text));
        }
        let at = pos2(x, rect.center().y - label.size().y / 2.0);
        ui.painter()
            .galley_with_override_text_color(at, label, fade(text));
        response
    }
}

/// A borderless square button showing `glyph`, named `name`; its hover text is the caller's.
pub(crate) fn icon_button(ui: &mut Ui, glyph: &str, name: &str) -> Response {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
    let colour = if response.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(RADIUS), p.hover);
        p.text
    } else {
        p.text2
    };
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        glyph,
        icons::font(18.0),
        colour,
    );
    response
}
