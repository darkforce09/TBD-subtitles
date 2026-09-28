use super::*;

#[test]
fn reads_release_and_git_versions() {
    let line = "ffmpeg version n8.1.3-20260926 Copyright (c) 2000-2026 the FFmpeg developers";
    assert_eq!(parse_version(line), Some((8, 1)));
    assert_eq!(parse_version("ffmpeg version 7.1 Copyright"), Some((7, 1)));
    assert_eq!(parse_version("ffmpeg version 9 Copyright"), Some((9, 0)));
    assert_eq!(parse_version("garbage"), None);
}

#[test]
fn finds_a_filter_or_device_by_its_row_name() {
    let filters = " T. apad              A->A       Pad audio with silence.\n \
                   .. scdet             V->V       Detect video scene change\n";
    assert!(lists(filters, "apad"));
    assert!(lists(filters, "scdet"));
    assert!(!lists(filters, "silence."));
    let devices = " DE pulse           Pulse audio output\n  E alsa            ALSA audio output\n";
    assert!(lists(devices, "pulse"));
    assert!(!lists(devices, "oss"));
}

#[test]
fn the_pin_is_a_sha256() {
    assert_eq!(FFMPEG_ARCHIVE.sha256.len(), 64);
    assert!(FFMPEG_ARCHIVE.url.ends_with(".tar.xz"));
}
