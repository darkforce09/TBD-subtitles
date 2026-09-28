//! The window's colours in light and dark, taken from the approved mockup's tokens.
//!
//! **Role:** name every colour the window paints, once per scheme, so the theme and the features
//! agree.
//!
//! **Position:** read by `core::ui::theme` to build egui's visuals, and by each feature's `ui`
//! through [`palette`] for text that carries meaning (secondary, good, warning, bad).
//!
//! **Signals and state:** constants only.
//!
//! **Invariants:** every text colour reads at a contrast of at least 4.5 on the surfaces it is
//! drawn on, the accent reads on the selection fill, and white reads on the accent fill
//! (`tests/palette.rs`).

use eframe::egui::{Color32, Ui};

/// The colours of one scheme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Palette {
    /// The window's background: panels and the page.
    pub(crate) window: Color32,
    /// The toolbar across the top of the window.
    pub(crate) toolbar: Color32,
    /// The sidebar that lists the videos.
    pub(crate) sidebar: Color32,
    /// Cards, popups and menus.
    pub(crate) card: Color32,
    /// The stripes of tables: the mockup's grouped grey in light, and in dark a grey lighter than
    /// the window, where the mockup's darker grouped grey would not show.
    pub(crate) stripe: Color32,
    /// Recessed wells: code and monospace blocks.
    pub(crate) well: Color32,
    /// The inside of buttons and text fields.
    pub(crate) control: Color32,
    /// The 1 px border of buttons and text fields.
    pub(crate) control_border: Color32,
    /// The fill of check boxes and slider rails.
    pub(crate) seg_bg: Color32,
    /// The chosen segment of a segmented control, raised above `seg_bg`.
    pub(crate) seg_on: Color32,
    /// Separators, the borders of cards, and the track of progress bars.
    pub(crate) line: Color32,
    /// Borders that must stand out: a hovered or pressed control.
    pub(crate) line_strong: Color32,
    /// Primary text.
    pub(crate) text: Color32,
    /// Secondary text: hints, empty states, file paths.
    pub(crate) text2: Color32,
    /// Quiet marks: the icons of waiting and cancelled jobs.
    pub(crate) text3: Color32,
    /// Links, selected text, the focus ring of text fields.
    pub(crate) accent: Color32,
    /// The accent as a fill: progress bars, the text cursor, and the fill white text sits on.
    pub(crate) accent_fill: Color32,
    /// The accent laid over the window behind a selection (translucent).
    pub(crate) accent_tint: Color32,
    /// Something done or in order.
    pub(crate) good: Color32,
    /// Something to look at, which does not stop a job.
    pub(crate) warn: Color32,
    /// Something broken, which stops a job.
    pub(crate) bad: Color32,
    /// The fill of icons that mark something done, to look at, or broken (brighter than the
    /// text colours, as icons need less contrast).
    pub(crate) good_icon: Color32,
    pub(crate) warn_icon: Color32,
    pub(crate) bad_icon: Color32,
    /// Laid over a control while the pointer is on it.
    pub(crate) hover: Color32,
    /// Laid over a control while it is pressed.
    pub(crate) press: Color32,
    /// The soft shadow under popups, menus and windows.
    pub(crate) shadow: Color32,
    /// The faint shadow under cards.
    pub(crate) card_shadow: Color32,
}

impl Palette {
    /// The opaque fill behind selected rows, tabs and text: the accent tint over the window.
    pub(crate) fn selection_fill(&self) -> Color32 {
        self.window.blend(self.accent_tint)
    }
}

/// The light scheme (the mockup's `:root`).
pub(crate) static LIGHT: Palette = Palette {
    window: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    toolbar: Color32::from_rgb(0xF7, 0xF7, 0xF8),
    sidebar: Color32::from_rgb(0xEF, 0xEF, 0xF2),
    card: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    stripe: Color32::from_rgb(0xF4, 0xF4, 0xF6),
    well: Color32::from_rgb(0xF4, 0xF4, 0xF6),
    control: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    control_border: Color32::from_rgb(0xCF, 0xCF, 0xD6),
    seg_bg: Color32::from_rgb(0xE6, 0xE6, 0xEA),
    seg_on: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    line: Color32::from_rgb(0xDE, 0xDE, 0xE3),
    line_strong: Color32::from_rgb(0xC9, 0xC9, 0xD0),
    text: Color32::from_rgb(0x1D, 0x1D, 0x1F),
    text2: Color32::from_rgb(0x5C, 0x5C, 0x63),
    text3: Color32::from_rgb(0x74, 0x74, 0x7B),
    accent: Color32::from_rgb(0x00, 0x64, 0xD2),
    accent_fill: Color32::from_rgb(0x00, 0x68, 0xDA),
    accent_tint: Color32::from_rgba_unmultiplied_const(0x00, 0x68, 0xDA, 28),
    good: Color32::from_rgb(0x1B, 0x7F, 0x37),
    warn: Color32::from_rgb(0xA6, 0x4E, 0x00),
    bad: Color32::from_rgb(0xC4, 0x16, 0x1C),
    good_icon: Color32::from_rgb(0x28, 0xA7, 0x45),
    warn_icon: Color32::from_rgb(0xE8, 0x84, 0x00),
    bad_icon: Color32::from_rgb(0xE5, 0x30, 0x2A),
    hover: Color32::from_rgba_unmultiplied_const(0, 0, 0, 13),
    press: Color32::from_rgba_unmultiplied_const(0, 0, 0, 23),
    shadow: Color32::from_rgba_unmultiplied_const(0, 0, 0, 41),
    card_shadow: Color32::from_rgba_unmultiplied_const(0, 0, 0, 18),
};

/// The dark scheme (the mockup's `[data-theme="dark"]`).
pub(crate) static DARK: Palette = Palette {
    window: Color32::from_rgb(0x1E, 0x1E, 0x20),
    toolbar: Color32::from_rgb(0x26, 0x26, 0x28),
    sidebar: Color32::from_rgb(0x23, 0x23, 0x25),
    card: Color32::from_rgb(0x2A, 0x2A, 0x2D),
    stripe: Color32::from_rgb(0x26, 0x26, 0x29),
    well: Color32::from_rgb(0x23, 0x23, 0x26),
    control: Color32::from_rgb(0x35, 0x35, 0x39),
    control_border: Color32::from_rgb(0x4A, 0x4A, 0x50),
    seg_bg: Color32::from_rgb(0x33, 0x33, 0x36),
    seg_on: Color32::from_rgb(0x5A, 0x5A, 0x60),
    line: Color32::from_rgb(0x38, 0x38, 0x3C),
    line_strong: Color32::from_rgb(0x4A, 0x4A, 0x50),
    text: Color32::from_rgb(0xF2, 0xF2, 0xF4),
    text2: Color32::from_rgb(0xAB, 0xAB, 0xB2),
    text3: Color32::from_rgb(0x8E, 0x8E, 0x96),
    accent: Color32::from_rgb(0x4D, 0xA2, 0xFF),
    accent_fill: Color32::from_rgb(0x15, 0x70, 0xDD),
    accent_tint: Color32::from_rgba_unmultiplied_const(0x4D, 0xA2, 0xFF, 41),
    good: Color32::from_rgb(0x4C, 0xD9, 0x64),
    warn: Color32::from_rgb(0xFF, 0xB3, 0x40),
    bad: Color32::from_rgb(0xFF, 0x6B, 0x63),
    good_icon: Color32::from_rgb(0x34, 0xC7, 0x59),
    warn_icon: Color32::from_rgb(0xFF, 0x9F, 0x0A),
    bad_icon: Color32::from_rgb(0xFF, 0x45, 0x3A),
    hover: Color32::from_rgba_unmultiplied_const(255, 255, 255, 15),
    press: Color32::from_rgba_unmultiplied_const(255, 255, 255, 26),
    shadow: Color32::from_rgba_unmultiplied_const(0, 0, 0, 128),
    card_shadow: Color32::from_rgba_unmultiplied_const(0, 0, 0, 72),
};

/// The palette of the scheme `ui` is drawn in.
pub(crate) fn palette(ui: &Ui) -> &'static Palette {
    if ui.visuals().dark_mode {
        &DARK
    } else {
        &LIGHT
    }
}

#[cfg(test)]
#[path = "tests/palette.rs"]
pub(crate) mod tests;
