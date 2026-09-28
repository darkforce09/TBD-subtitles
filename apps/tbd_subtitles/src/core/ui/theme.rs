//! The window's theme: text sizes, spacing, radii, borders and shadow, and egui's visuals for the
//! light and the dark palette.
//!
//! **Role:** install the look once when the window opens, and switch between light and dark as
//! the desktop does.
//!
//! **Position:** `install` runs when the window (or a test harness) is created; `follow` runs at
//! the top of every frame with the scheme `core::color_scheme` reported.
//!
//! **Signals and state:** writes egui's styles and theme preference in the context; `follow`
//! asks for one more frame when it switches.
//!
//! **Invariants:** the text styles name only families `core::ui::fonts` registers, so `install`
//! installs the fonts too; both themes share every size and differ only in colour.

use std::collections::BTreeMap;

use eframe::egui::{
    Context, CornerRadius, FontFamily, FontId, Margin, Shadow, Spacing, Stroke, TextStyle, Theme,
    ThemePreference, Visuals, style::WidgetVisuals, vec2,
};

use crate::core::color_scheme::Scheme;
use crate::core::ui::fonts;
use crate::core::ui::palette::{DARK, LIGHT, Palette};

/// The corner radius of buttons, fields and menus.
const CONTROL_RADIUS: u8 = 6;
/// The corner radius of cards and windows.
const CARD_RADIUS: u8 = 10;
/// The name of the 22 px bold text style: page titles.
pub(crate) const TITLE: &str = "title";

/// Install the fonts, sizes and both themes' visuals in `ctx`.
pub(crate) fn install(ctx: &Context) {
    fonts::install(ctx);
    ctx.all_styles_mut(|style| {
        style.text_styles = text_styles();
        spacing(&mut style.spacing);
    });
    ctx.set_visuals_of(Theme::Light, visuals(&LIGHT, false));
    ctx.set_visuals_of(Theme::Dark, visuals(&DARK, true));
}

/// Draw in the theme of `scheme`; a switch shows from the next frame, which it asks for.
pub(crate) fn follow(ctx: &Context, scheme: Scheme) {
    let theme = match scheme {
        Scheme::Light => Theme::Light,
        Scheme::Dark => Theme::Dark,
    };
    let preference = ThemePreference::from(theme);
    if ctx.options(|options| options.theme_preference) != preference {
        ctx.set_theme(preference);
        ctx.request_repaint();
    }
}

/// Title 22 bold ([`TITLE`]), headline 15 semibold (`Heading`, so `ui.heading` heads a
/// section), body 13, caption 11 (`Small`).
fn text_styles() -> BTreeMap<TextStyle, FontId> {
    let bold = FontFamily::Name(fonts::BOLD.into());
    let semibold = FontFamily::Name(fonts::SEMIBOLD.into());
    BTreeMap::from([
        (TextStyle::Name(TITLE.into()), FontId::new(22.0, bold)),
        (TextStyle::Heading, FontId::new(15.0, semibold)),
        (TextStyle::Body, FontId::proportional(13.0)),
        (TextStyle::Button, FontId::proportional(13.0)),
        (TextStyle::Small, FontId::proportional(11.0)),
        (TextStyle::Monospace, FontId::monospace(12.0)),
    ])
}

/// Controls 28 px high with 12 px of side padding, as the mockup's buttons and fields.
fn spacing(spacing: &mut Spacing) {
    spacing.item_spacing = vec2(8.0, 6.0);
    spacing.button_padding = vec2(12.0, 4.0);
    spacing.interact_size = vec2(40.0, 28.0);
    spacing.window_margin = Margin::same(12);
    spacing.menu_margin = Margin::same(6);
    spacing.indent = 16.0;
}

/// egui's visuals painted with `palette`.
fn visuals(palette: &Palette, dark: bool) -> Visuals {
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let border = |color| Stroke::new(1.0, color);
    let shadow = Shadow {
        offset: [0, 8],
        blur: 28,
        spread: 0,
        color: palette.shadow,
    };
    visuals.panel_fill = palette.window;
    visuals.window_fill = palette.card;
    visuals.window_stroke = border(palette.line);
    visuals.window_corner_radius = CornerRadius::same(CARD_RADIUS);
    visuals.menu_corner_radius = CornerRadius::same(CONTROL_RADIUS);
    visuals.window_shadow = shadow;
    visuals.popup_shadow = shadow;
    visuals.faint_bg_color = palette.stripe;
    // Progress bar tracks and scroll bar rails; text fields keep the control fill.
    visuals.extreme_bg_color = palette.line;
    visuals.text_edit_bg_color = Some(palette.control);
    visuals.code_bg_color = palette.well;
    visuals.hyperlink_color = palette.accent;
    visuals.warn_fg_color = palette.warn;
    visuals.error_fg_color = palette.bad;
    visuals.weak_text_color = Some(palette.text2);
    // As macOS: a light accent tint behind selections, the accent for selected text and for the
    // focus ring of a text field (egui draws both with the selection stroke).
    visuals.selection.bg_fill = palette.selection_fill();
    visuals.selection.stroke = Stroke::new(1.0, palette.accent);
    visuals.text_cursor.stroke = Stroke::new(2.0, palette.accent_fill);
    let widget = |fill, weak_fill, stroke| WidgetVisuals {
        bg_fill: fill,
        weak_bg_fill: weak_fill,
        bg_stroke: border(stroke),
        corner_radius: CornerRadius::same(CONTROL_RADIUS),
        fg_stroke: Stroke::new(1.0, palette.text),
        expansion: 0.0,
    };
    let widgets = &mut visuals.widgets;
    widgets.noninteractive = widget(palette.window, palette.window, palette.line);
    widgets.inactive = widget(palette.seg_bg, palette.control, palette.control_border);
    widgets.hovered = widget(
        palette.seg_bg.blend(palette.hover),
        palette.control.blend(palette.hover),
        palette.line_strong,
    );
    widgets.active = widget(
        palette.seg_bg.blend(palette.press),
        palette.control.blend(palette.press),
        palette.line_strong,
    );
    widgets.open = widgets.hovered;
    visuals
}

#[cfg(test)]
#[path = "tests/theme.rs"]
mod tests;
