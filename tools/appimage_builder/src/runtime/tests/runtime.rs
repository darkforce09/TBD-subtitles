use super::*;

#[test]
fn names_the_output_by_version_and_commit() {
    assert_eq!(
        versioned_name("0.1.0", "9b528e6"),
        "TBD-subtitles-0.1.0-9b528e6-x86_64.AppImage"
    );
    assert_eq!(stable_name(), "TBD-subtitles-x86_64.AppImage");
}

#[test]
fn reads_the_app_version_line() {
    assert_eq!(parse_app_version("tbd-subtitles 0.1.0\n"), Some("0.1.0"));
    assert_eq!(parse_app_version("something 0.1.0"), None);
    assert_eq!(parse_app_version("tbd-subtitles"), None);
}

#[test]
fn finishing_marks_executable_and_copies_to_the_stable_name() {
    let dir = std::env::temp_dir().join(format!("appimage_builder-finish-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let image = dir.join(versioned_name("0.1.0", "abc1234"));
    fs::write(&image, b"image").unwrap();
    let stable = finish(&image).unwrap();
    assert_eq!(stable, dir.join(stable_name()));
    assert_eq!(fs::read(&stable).unwrap(), b"image");
    let mode = fs::metadata(&image).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o755);
    fs::remove_dir_all(dir).unwrap();
}
