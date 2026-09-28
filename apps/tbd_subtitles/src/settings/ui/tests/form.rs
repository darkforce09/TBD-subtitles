use std::path::Path;

use super::*;

#[test]
fn a_typed_number_must_be_finite_and_in_range() {
    let range = 1.0..=100.0;
    assert_eq!(typed_number(" 25 ", &range), Some(25.0));
    assert_eq!(typed_number("25.5", &range), Some(25.5));
    for bad in [
        "nan", "NaN", "inf", "-inf", "infinity", "0", "0.5", "101", "1e9", "", "twenty",
    ] {
        assert_eq!(typed_number(bad, &range), None, "{bad}");
    }
}

#[test]
fn the_home_folder_reads_as_a_tilde() {
    let home = Some(Path::new("/home/owner"));
    assert_eq!(
        tilde_under(
            Path::new("/home/owner/.local/share/tbd-subtitles/models"),
            home
        ),
        "~/.local/share/tbd-subtitles/models"
    );
    assert_eq!(tilde_under(Path::new("/home/owner"), home), "~");
    assert_eq!(
        tilde_under(Path::new("/home/ownerx/models"), home),
        "/home/ownerx/models"
    );
    assert_eq!(tilde_under(Path::new("/mnt/models"), None), "/mnt/models");
}
