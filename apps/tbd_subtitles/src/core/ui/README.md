# Shared window look

The look every feature's UI shares, taken from the approved mockup: the light and dark palettes,
the fonts, the theme built from them, and the widgets drawn the same everywhere (buttons, cards,
disclosures, progress bars, segmented controls, icons and status marks, toasts), so panels drawn
by different features look alike.

## Contents

```text
apps/tbd_subtitles/src/core/ui/
├── button.rs      `Button` (bordered, blue primary or red danger; 24, 28 or 32 px) and `icon_button`
├── card.rs        `card`, `card_head`, `card_text` and `well_text`: the raised card and its parts
├── disclosure.rs  `disclosure`: a full-width row with a turning chevron, open or closed in memory
├── fonts.rs       `definitions` and `install`: Adwaita Sans in three weights, Adwaita Mono, Phosphor
├── icons.rs       the Phosphor glyphs by name, `font`, and the painted status marks (`StatusIcon`)
├── mod.rs         the module tree
├── palette.rs     `Palette`, `LIGHT` and `DARK` from the mockup's tokens; `palette(ui)` picks one
├── progress.rs    `bar` and `paint_bar`: the rounded progress bar, 4 px or 6 px thick
├── segmented.rs   `segmented`: a row of segments with the chosen one raised
├── tests/         unit tests for the palette's contrast, the font families and the theme's visuals
├── theme.rs       `install`: text styles, spacing, radii, borders, shadow, visuals; `follow`: light or dark
└── toast.rs       `toasts_ui`: the toasts at the bottom centre, each with its mark, text and button
```

## How it works

`theme::install` runs once when the window opens (and in each test harness). It installs the
fonts first: `fonts::install` reads `/usr/share/fonts/adwaita-sans-fonts/AdwaitaSans-Regular.ttf`
and `/usr/share/fonts/adwaita-mono-fonts/AdwaitaMono-Regular.ttf`, and `fonts::definitions` puts
them in front of egui's own fonts: Adwaita Sans once at weight 400 as the proportional font and
twice more with the `wght` axis set to 600 and 700 as the families `semibold` and `bold`, Adwaita
Mono as the monospace font ahead of egui's Hack. The Phosphor icon font (crate `egui-phosphor`,
regular weight) is a fallback of the proportional, `semibold`, `bold` and monospace families, so
icon glyphs draw inside their text; alone it is the family `phosphor`, in which every icon is
drawn (`icons::font`), since Adwaita Sans has glyphs of its own at some of the icon font's
private-use code points. Without the system fonts each family falls back to egui's fonts. Then it sets the text styles (`Heading` is the 15 px semibold headline, so `ui.heading`
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
secondary text, `good`, `warn` and `bad` for states; the toolbar and the sidebar have their own
greys, and icons their brighter `good_icon`, `warn_icon`, `bad_icon` and the quiet `text3`. The
accent is macOS blue as mocked; KDE's accent colour is not followed.

`button::Button` paints the mockup's button from the palette: a bordered control, the blue
primary one with white text, or the danger one with red text (Cancel, Remove from List), 24, 28
or 32 px high, an optional icon before the label, a hover and
a pressed look, 45 % opacity when disabled (it then senses no click), and its label as its
accessible name. `icon_button` is a borderless 28 px square with an 18 px glyph. `icons` names the
glyphs the window uses and paints the status marks on a 24-unit grid: a clock (waiting), a progress
ring (running), an empty grey ring (still to come), a white check on green (done), a white exclamation on an orange triangle (needs a
look), a white cross on red (failed), a stop (cancelled), a turning arc (working) and an info mark;
on a selected row every mark is white. `toast::toasts_ui` draws the toasts at the bottom centre,
the oldest lowest, each a card with its mark, its text and a small button; an error's border and
text are red. It returns the toast whose button was pressed.

`card::card` draws the mockup's card: the card fill, a 1 px border, radius 10 and a faint shadow
(`card_shadow`), as wide as the space given, with 16 px above and below and 18 px beside its
contents and 12 px between them when padded, or none (for a card of rows). `card_head` puts a 28
px status mark beside a 15 px semibold title over lines of text 3 px apart, `card_text` writes a
wrapped line in a given colour, and `well_text` a path or a raw message in the monospace font on
the recessed well (cut with an ellipsis, or wrapped). `disclosure::disclosure` draws a full-width
row 42 px high: a chevron that points right, or down while open, a semibold label that names the
row for accessibility, and a grey note on the right; a click opens or closes it, kept in egui's
memory under the caller's id. `progress::bar` draws a bar of a given width filled to a share, 4 px
high, or 6 px thick, the track in the separator grey and the fill in `accent_fill`, named as a
progress indicator; `paint_bar` paints one into a given rectangle. `segmented::segmented` draws a
row of 24 px segments in a 2 px `seg_bg` well, radius 8, the chosen one raised in `seg_on` with a
small shadow; each segment is named with whether it is chosen, and a click on another returns its
value.

## Public surface

- `theme::{install, follow, TITLE}`; `fonts::{definitions, install, SEMIBOLD, BOLD, ICONS}`;
  `palette::{palette, Palette, LIGHT, DARK}`; `button::{Button, ButtonSize, icon_button}`;
  `card::{card, card_head, card_text, well_text}`; `disclosure::disclosure`;
  `progress::{bar, paint_bar}`; `segmented::segmented`;
  `icons::{font, StatusIcon, status_icon, paint_status}` and the glyph names; `toast::toasts_ui`.

## Boundaries

- Depends on: `eframe::egui` (colours, fonts, styles, painting); `egui-phosphor` in `fonts.rs` and
  `icons.rs`; `crate::core::color_scheme::Scheme` in `theme.rs`; `crate::core::toast` in
  `toast.rs`.
- Used by: `crate::application` (`launch` installs the theme, `App::ui` follows the scheme, the
  frame draws the toasts) and every feature's `ui` for the palette and the widgets.
- Rules:
  - nothing here imports a feature or a composition module
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a feature may use this folder though it
    may not use another feature's `ui`;
  - every text colour reads at a contrast of at least 4.5 on the surfaces it is drawn on (text on
    a chosen segment too), the accent and the warning colour read on the selection, white reads
    on the accent fill, and table stripes show (`every_text_colour_reads_on_every_surface`,
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
    system fonts, and icons draw from the icon font first
    (`the_families_exist_without_the_system_fonts`,
    `the_system_fonts_lead_each_family_in_its_weight`,
    `icons_draw_from_the_icon_font_ahead_of_adwaita_sans` in `tests/fonts.rs`);
  - text reads on the sidebar and the toolbar (`sidebar_and_toolbar_text_reads_on_its_bar` in
    `tests/palette.rs`).
