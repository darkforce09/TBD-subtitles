use super::*;

#[test]
fn the_desktop_entry_names_the_app_icon_and_window_class() {
    for line in [
        "[Desktop Entry]",
        "Exec=tbd-subtitles %F",
        "Icon=tbd-subtitles",
        "MimeType=video/mp4;video/x-matroska;video/webm;video/quicktime;video/x-msvideo;video/mp2t;video/x-m4v;",
        "StartupWMClass=tbd-subtitles",
        "Categories=AudioVideo;Video;",
        "Terminal=false",
    ] {
        assert!(DESKTOP_ENTRY.lines().any(|l| l == line), "missing {line}");
    }
}

#[test]
fn the_icon_is_a_square_png_with_transparent_corners() {
    let bytes = icon_png(64).unwrap();
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (64, 64));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(pixels[3], 0, "the top-left corner is transparent");
    let centre = ((32 * 64 + 32) * 4) as usize;
    assert_eq!(pixels[centre + 3], 255, "the centre is opaque");
    assert_eq!(icon_png(64).unwrap(), icon_png(64).unwrap());
}

#[test]
fn finish_lays_out_the_root_of_the_app_dir() {
    let root = std::env::temp_dir().join(format!("appimage_builder-appdir-{}", std::process::id()));
    let app_dir = AppDir::create(&root).unwrap();
    app_dir.finish().unwrap();
    assert!(root.join("usr/bin").is_dir());
    assert!(root.join("usr/lib").is_dir());
    assert!(root.join("tbd-subtitles.desktop").is_file());
    assert!(
        root.join("usr/share/applications/tbd-subtitles.desktop")
            .is_file()
    );
    assert!(
        root.join("usr/share/icons/hicolor/256x256/apps/tbd-subtitles.png")
            .is_file()
    );
    assert_eq!(
        fs::read_link(root.join("AppRun")).unwrap(),
        PathBuf::from("usr/bin/tbd-subtitles")
    );
    assert_eq!(
        fs::read_link(root.join(".DirIcon")).unwrap(),
        PathBuf::from("tbd-subtitles.png")
    );
    AppDir::create(&root).unwrap();
    assert!(!root.join("AppRun").exists());
    fs::remove_dir_all(root).unwrap();
}

/// FNV-1a over the bytes: a fingerprint that stays the same across Rust releases.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[test]
fn the_icon_png_bytes_match_the_shipped_icon() {
    for (size, length, fingerprint) in [
        (16, 243, 0xcadf_8e2c_d2f8_3cf2_u64),
        (64, 704, 0x50eb_f7bb_12f4_afcf),
        (ICON_SIZE, 2822, 0x0edb_9aa3_116e_163d),
    ] {
        let png = icon_png(size).unwrap();
        assert_eq!(png.len(), length, "size {size}");
        assert_eq!(fnv1a(&png), fingerprint, "size {size}");
    }
}
