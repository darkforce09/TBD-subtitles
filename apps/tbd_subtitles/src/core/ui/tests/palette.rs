use super::*;

/// WCAG relative luminance of an opaque colour.
fn luminance(colour: Color32) -> f32 {
    let channel = |value: u8| {
        let c = f32::from(value) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
}

/// WCAG contrast ratio of two opaque colours, from 1 to 21.
pub(crate) fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

fn schemes() -> [(&'static str, &'static Palette); 2] {
    [("light", &LIGHT), ("dark", &DARK)]
}

#[test]
fn every_text_colour_reads_on_every_surface() {
    for (scheme, p) in schemes() {
        let surfaces = [
            ("window", p.window),
            ("card", p.card),
            ("stripe", p.stripe),
            ("well", p.well),
        ];
        let texts = [
            ("text", p.text),
            ("text2", p.text2),
            ("accent", p.accent),
            ("good", p.good),
            ("warn", p.warn),
            ("bad", p.bad),
        ];
        for (surface, background) in surfaces {
            for (name, text) in texts {
                let ratio = contrast(text, background);
                assert!(ratio >= 4.5, "{scheme}: {name} on {surface} is {ratio:.2}");
            }
        }
    }
}

#[test]
fn control_text_reads_on_controls_and_tracks() {
    for (scheme, p) in schemes() {
        for (surface, background) in [("control", p.control), ("seg_bg", p.seg_bg)] {
            for (name, text) in [("text", p.text), ("text2", p.text2)] {
                let ratio = contrast(text, background);
                assert!(ratio >= 4.5, "{scheme}: {name} on {surface} is {ratio:.2}");
            }
        }
        let ratio = contrast(p.text, p.seg_on);
        assert!(ratio >= 4.5, "{scheme}: text on seg_on is {ratio:.2}");
    }
}

#[test]
fn selected_text_and_its_marks_read_on_the_selection() {
    for (scheme, p) in schemes() {
        for (name, text) in [("accent", p.accent), ("warn", p.warn)] {
            let ratio = contrast(text, p.selection_fill());
            assert!(
                ratio >= 4.5,
                "{scheme}: {name} on the selection is {ratio:.2}"
            );
        }
    }
}

#[test]
fn white_reads_on_the_accent_fill() {
    for (scheme, p) in schemes() {
        let ratio = contrast(Color32::WHITE, p.accent_fill);
        assert!(ratio >= 4.5, "{scheme}: white on accent_fill is {ratio:.2}");
    }
}

#[test]
fn table_stripes_show_on_the_window() {
    for (scheme, p) in schemes() {
        let ratio = contrast(p.stripe, p.window);
        assert!(ratio >= 1.08, "{scheme}: stripe on window is {ratio:.3}");
    }
}

#[test]
fn the_mockup_accent_fills_are_kept() {
    assert_eq!(LIGHT.accent_fill, Color32::from_rgb(0x00, 0x68, 0xDA));
    assert_eq!(DARK.accent_fill, Color32::from_rgb(0x15, 0x70, 0xDD));
}

#[test]
fn sidebar_and_toolbar_text_reads_on_its_bar() {
    for (scheme, p) in schemes() {
        for (surface, background) in [("sidebar", p.sidebar), ("toolbar", p.toolbar)] {
            for (name, text) in [("text", p.text), ("text2", p.text2), ("bad", p.bad)] {
                let ratio = contrast(text, background);
                assert!(ratio >= 4.5, "{scheme}: {name} on {surface} is {ratio:.2}");
            }
        }
    }
}
