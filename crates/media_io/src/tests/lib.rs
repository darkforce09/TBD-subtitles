//! Tests for `Programs::beside`: picks the bundled pair when both files exist, else bare names.

use std::fs;

use crate::Programs;

#[test]
fn beside_picks_bundled_pair_when_both_exist() {
    let dir = std::env::temp_dir().join(format!("tbd-media-io-test-{}", std::process::id()));
    let ffmpeg_dir = dir.join("ffmpeg");
    fs::create_dir_all(&ffmpeg_dir).unwrap();
    fs::write(ffmpeg_dir.join("ffmpeg"), b"").unwrap();
    fs::write(ffmpeg_dir.join("ffprobe"), b"").unwrap();

    let programs = Programs::beside(&dir);

    assert!(programs.bundled);
    assert_eq!(programs.ffmpeg, ffmpeg_dir.join("ffmpeg").to_string_lossy());
    assert_eq!(
        programs.ffprobe,
        ffmpeg_dir.join("ffprobe").to_string_lossy()
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn beside_falls_back_to_bare_names_when_missing() {
    let dir =
        std::env::temp_dir().join(format!("tbd-media-io-test-missing-{}", std::process::id()));
    fs::create_dir_all(&dir).ok();

    let programs = Programs::beside(&dir);

    assert!(!programs.bundled);
    assert_eq!(programs.ffmpeg, "ffmpeg");
    assert_eq!(programs.ffprobe, "ffprobe");

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn beside_falls_back_when_only_one_present() {
    let dir =
        std::env::temp_dir().join(format!("tbd-media-io-test-partial-{}", std::process::id()));
    let ffmpeg_dir = dir.join("ffmpeg");
    fs::create_dir_all(&ffmpeg_dir).unwrap();
    fs::write(ffmpeg_dir.join("ffmpeg"), b"").unwrap();

    let programs = Programs::beside(&dir);

    assert!(!programs.bundled);
    assert_eq!(programs.ffmpeg, "ffmpeg");

    fs::remove_dir_all(&dir).ok();
}
