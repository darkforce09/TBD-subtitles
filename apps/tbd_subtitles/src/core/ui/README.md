# Shared window look

The look every feature's UI shares, taken from the approved mockup: the light and dark palettes,
the fonts, and the theme built from them, so panels drawn by different features look alike. It
holds no widgets; each feature draws its own.

## Contents

```text
apps/tbd_subtitles/src/core/ui/
├── fonts.rs    `definitions` and `install`: Adwaita Sans in three weights, Adwaita Mono, Phosphor icons
├── mod.rs      the module tree
├── palette.rs  `Palette`, `LIGHT` and `DARK` from the mockup's tokens; `palette(ui)` picks one
├── tests/      unit tests for the palette's contrast, the font families and the theme's visuals
└── theme.rs    `install`: text styles, spacing, radii, borders, shadow, visuals; `follow`: light or dark
```

## How it works

`theme::install` runs once when the window opens (and in each test harness). It installs the
fonts first: `fonts::install` reads `/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf`
and `/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf`, and `fonts::definitions` puts
them in front of egui's own fonts: Adwaita Sans once at weight 400 as the proportional font and
twice more with the `wght` axis set to 600 and 700 as the families `semibold` and `bold`, Adwaita
Mono as the monospace font ahead of egui's Hack. The Phosphor icon font (crate `egui-phosphor`,
regular weight) is a fallback of the proportional, `semibold`, `bold` and monospace families, so
icon glyphs draw inside their text. Without the system fonts each family falls back to egui's
fonts. Then it sets the text styles (`Heading` is the 15 px semibold headline, so `ui.heading`
heads a section; the named style `title` is the 22 px bold page title; body and buttons 13;
caption 11 as `Small`), controls 28 px high with 12 px of side padding, radius 6 on controls and
10 on cards and windows, 1 px borders, one soft shadow, and egui's visuals for the light and the
dark theme from the two palettes. Selections are drawn as on macOS: the accent tint behind them
and the accent for selected text and for a focused text field's ring. Progress bar tracks take
the separator grey, and table stripes the palette's stripe. Each feature fills its progress bars
with `accent_fill` and writes any percentage beside the bar, never inside it.

`theme::follow` runs at the top of every frame with the scheme `core::color_scheme` reported; when
it differs from the theme in use it switches and asks for one more frame, since a frame already
started keeps its style.

Each feature takes the colours of text that carries meaning from `palette(ui)`: `text2` for
secondary text, `good`, `warn` and `bad` for states. The accent is macOS blue as mocked; KDE's
accent colour is not followed.

## Public surface

- `theme::{install, follow, TITLE}`; `fonts::{definitions, install, SEMIBOLD, BOLD}`;
  `palette::{palette, Palette, LIGHT, DARK}`.

## Boundaries

- Depends on: `eframe::egui` (colours, fonts, styles); `egui-phosphor` in `fonts.rs`;
  `crate::core::color_scheme::Scheme` in `theme.rs`.
- Used by: `crate::application` (`launch` installs the theme, `App::ui` follows the scheme) and
  every feature's `ui` for the palette.
- Rules:
  - nothing here imports a feature or a composition module
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a feature may use this folder though it
    may not use another feature's `ui`;
  - every text colour reads at a contrast of at least 4.5 on the surfaces it is drawn on, the
    accent and the warning colour read on the selection, white reads on the accent fill, and
    table stripes show (`every_text_colour_reads_on_every_surface`,
    `control_text_reads_on_controls_and_tracks`,
    `selected_text_and_its_marks_read_on_the_selection`, `white_reads_on_the_accent_fill`,
    `table_stripes_show_on_the_window` in `tests/palette.rs`);
  - a focused text field's ring stands out from its fill at 3:1 or more, selected text reads on
    the selection, progress tracks show on the window, and `Heading` is the headline
    (`a_focused_text_field_shows_its_ring_on_the_control_fill`,
    `selected_text_reads_on_the_selection`,
    `progress_tracks_and_table_stripes_show_on_the_window`,
    `headings_are_headlines_and_titles_are_named` in `tests/theme.rs`);
  - the families `semibold`, `bold`, monospace and the icon font exist with or without the
    system fonts (`the_families_exist_without_the_system_fonts`,
    `the_system_fonts_lead_each_family_in_its_weight` in `tests/fonts.rs`).
