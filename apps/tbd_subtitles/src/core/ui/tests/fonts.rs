use super::*;

/// Every family the window names, with the icon font's.
fn families() -> [FontFamily; 5] {
    [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name(SEMIBOLD.into()),
        FontFamily::Name(BOLD.into()),
        FontFamily::Name(ICONS.into()),
    ]
}

fn assert_bound(fonts: &FontDefinitions) {
    for family in families() {
        let members = fonts
            .families
            .get(&family)
            .unwrap_or_else(|| panic!("{family:?} is missing"));
        assert!(!members.is_empty(), "{family:?} has no font");
        for member in members {
            assert!(
                fonts.font_data.contains_key(member),
                "{family:?} names {member}, which is not loaded"
            );
        }
    }
}

/// Any TTF stands in for the system fonts, which the build container lacks.
fn stand_in() -> Vec<u8> {
    FontDefinitions::default().font_data["Ubuntu-Light"]
        .font
        .to_vec()
}

#[test]
fn the_families_exist_without_the_system_fonts() {
    let fonts = definitions(None, None);
    assert_bound(&fonts);
    assert_eq!(fonts.families[&FontFamily::Monospace][0], "Hack");
    for family in [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name(SEMIBOLD.into()),
        FontFamily::Name(BOLD.into()),
    ] {
        assert!(
            fonts.families[&family].contains(&ICONS.to_string()),
            "{family:?} draws icons"
        );
    }
}

#[test]
fn the_system_fonts_lead_each_family_in_its_weight() {
    let fonts = definitions(Some(stand_in()), Some(stand_in()));
    assert_bound(&fonts);
    for (family, first) in [
        (FontFamily::Proportional, "adwaita-sans-400"),
        (FontFamily::Name(SEMIBOLD.into()), "adwaita-sans-600"),
        (FontFamily::Name(BOLD.into()), "adwaita-sans-700"),
        (FontFamily::Monospace, "adwaita-mono"),
    ] {
        assert_eq!(fonts.families[&family][0], first, "{family:?}");
    }
    assert_eq!(
        fonts.families[&FontFamily::Monospace][1],
        "Hack",
        "Hack follows Adwaita Mono"
    );
    assert!(fonts.families[&FontFamily::Monospace].contains(&ICONS.to_string()));
}
