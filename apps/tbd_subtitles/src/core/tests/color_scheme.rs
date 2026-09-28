use super::*;

#[test]
fn only_a_dark_preference_makes_the_window_dark() {
    assert_eq!(Scheme::from_portal(ColorScheme::PreferDark), Scheme::Dark);
    assert_eq!(Scheme::from_portal(ColorScheme::PreferLight), Scheme::Light);
    assert_eq!(
        Scheme::from_portal(ColorScheme::NoPreference),
        Scheme::Light
    );
}

#[test]
fn the_window_starts_light() {
    assert_eq!(Scheme::default(), Scheme::Light);
}
