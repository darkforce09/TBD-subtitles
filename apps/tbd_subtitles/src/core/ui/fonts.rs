//! The window's fonts: the desktop's Adwaita Sans in three weights and Adwaita Mono, with the
//! Phosphor icon font.
//!
//! **Role:** build egui's font definitions and install them: Adwaita Sans regular as the
//! proportional font, semibold (600) and bold (700) as the families [`SEMIBOLD`] and [`BOLD`],
//! Adwaita Mono as the monospace font, and the icon font as a fallback of those four families
//! and alone as the family [`ICONS`].
//!
//! **Position:** installed by `core::ui::theme::install`, whose text styles name these families.
//!
//! **Signals and state:** reads the two system font files once; no other state.
//!
//! **Invariants:** the families `semibold`, `bold` and the icon font always exist, and the icon
//! family draws from the icon font first; without the system fonts every family falls back to
//! egui's own fonts (`tests/fonts.rs`).

use std::sync::Arc;

use eframe::egui::epaint::text::VariationCoords;
use eframe::egui::{Context, FontData, FontDefinitions, FontFamily, FontTweak};

/// Adwaita Sans, a variable font with a weight axis, as Fedora installs it.
const SANS_FONT: &str = "/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf";
/// Adwaita Mono regular, as Fedora installs it.
const MONO_FONT: &str = "/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf";
/// The name egui-phosphor gives the icon font, and the family that draws icons alone: Adwaita
/// Sans has glyphs of its own at some of the icon font's private-use code points, so an icon is
/// drawn in this family, where the icon font comes first.
pub(crate) const ICONS: &str = "phosphor";
/// The family of semibold (600) text: headlines.
pub(crate) const SEMIBOLD: &str = "semibold";
/// The family of bold (700) text: titles.
pub(crate) const BOLD: &str = "bold";

/// egui's fonts with the icon font, and the system's `sans` and `mono` (the bytes of Adwaita Sans
/// and Adwaita Mono) in front of them when given.
pub(crate) fn definitions(sans: Option<Vec<u8>>, mono: Option<Vec<u8>>) -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
    let fallbacks = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let weights = [
        (FontFamily::Proportional, "adwaita-sans-400", None),
        (
            FontFamily::Name(SEMIBOLD.into()),
            "adwaita-sans-600",
            Some(600.0),
        ),
        (
            FontFamily::Name(BOLD.into()),
            "adwaita-sans-700",
            Some(700.0),
        ),
    ];
    for (family, name, weight) in weights {
        let mut members = fallbacks.clone();
        if let Some(bytes) = &sans {
            let mut data = FontData::from_owned(bytes.clone());
            if let Some(weight) = weight {
                data = data.tweak(FontTweak {
                    coords: VariationCoords::new([(b"wght", weight)]),
                    ..Default::default()
                });
            }
            fonts.font_data.insert(name.into(), Arc::new(data));
            members.insert(0, name.into());
        }
        fonts.families.insert(family, members);
    }
    fonts
        .families
        .insert(FontFamily::Name(ICONS.into()), vec![ICONS.into()]);
    let monospace = fonts.families.entry(FontFamily::Monospace).or_default();
    monospace.push(ICONS.into());
    if let Some(bytes) = mono {
        monospace.insert(0, "adwaita-mono".into());
        fonts
            .font_data
            .insert("adwaita-mono".into(), Arc::new(FontData::from_owned(bytes)));
    }
    fonts
}

/// Install the fonts in `ctx`, with Adwaita Sans and Adwaita Mono when the system has them.
pub(crate) fn install(ctx: &Context) {
    ctx.set_fonts(definitions(read(SANS_FONT), read(MONO_FONT)));
}

/// The bytes of the font file at `path`, or `None` (logged) when it is missing.
fn read(path: &str) -> Option<Vec<u8>> {
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) => {
            tracing::info!(%error, path, "a system font is missing; egui's fonts are used");
            None
        }
    }
}

#[cfg(test)]
#[path = "tests/fonts.rs"]
mod tests;
