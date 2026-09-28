use super::*;
use crate::core::ui::palette::tests::contrast;

fn themes() -> [(Theme, &'static Palette); 2] {
    [(Theme::Light, &LIGHT), (Theme::Dark, &DARK)]
}

#[test]
fn a_focused_text_field_shows_its_ring_on_the_control_fill() {
    for (theme, palette) in themes() {
        let visuals = visuals(palette, theme == Theme::Dark);
        let ring = visuals.selection.stroke.color;
        let field = visuals.text_edit_bg_color();
        let ratio = contrast(ring, field);
        // WCAG's 3:1 for the boundary of a control.
        assert!(
            ratio >= 3.0,
            "{theme:?}: focus ring on the field is {ratio:.2}"
        );
        assert_eq!(ring, palette.accent, "{theme:?}");
    }
}

#[test]
fn selected_text_reads_on_the_selection() {
    for (theme, palette) in themes() {
        let visuals = visuals(palette, theme == Theme::Dark);
        let ratio = contrast(visuals.selection.stroke.color, visuals.selection.bg_fill);
        assert!(ratio >= 4.5, "{theme:?}: selected text is {ratio:.2}");
    }
}

#[test]
fn progress_tracks_and_table_stripes_show_on_the_window() {
    for (theme, palette) in themes() {
        let visuals = visuals(palette, theme == Theme::Dark);
        assert_ne!(visuals.extreme_bg_color, visuals.panel_fill, "{theme:?}");
        assert_eq!(visuals.extreme_bg_color, palette.line, "{theme:?}");
        assert_eq!(visuals.faint_bg_color, palette.stripe, "{theme:?}");
    }
}

#[test]
fn headings_are_headlines_and_titles_are_named() {
    let styles = text_styles();
    assert_eq!(
        styles[&TextStyle::Heading],
        FontId::new(15.0, FontFamily::Name(fonts::SEMIBOLD.into()))
    );
    assert_eq!(
        styles[&TextStyle::Name(TITLE.into())],
        FontId::new(22.0, FontFamily::Name(fonts::BOLD.into()))
    );
    assert_eq!(styles[&TextStyle::Body], FontId::proportional(13.0));
    assert_eq!(styles[&TextStyle::Small], FontId::proportional(11.0));
}
