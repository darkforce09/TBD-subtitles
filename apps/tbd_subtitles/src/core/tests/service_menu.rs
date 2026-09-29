use super::*;

/// An empty folder of its own under the temporary folder.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-menu-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn exec_line(text: &str) -> &str {
    text.lines()
        .find(|line| line.starts_with("Exec="))
        .expect("an Exec line")
}

#[test]
fn the_menu_offers_one_action_for_videos() {
    let text = menu_text(
        Path::new("/apps/TBD.AppImage"),
        Path::new("/data/tbd-subtitles.png"),
    );
    for line in [
        "[Desktop Entry]",
        "Type=Service",
        "X-KDE-ServiceTypes=KonqPopupMenu/Plugin",
        "MimeType=video/*;",
        "Actions=generateSubtitles",
        "[Desktop Action generateSubtitles]",
        "Name=Generate subtitles",
        "Icon=/data/tbd-subtitles.png",
    ] {
        assert!(text.lines().any(|l| l == line), "missing {line}");
    }
    assert_eq!(
        exec_line(&text),
        "Exec=\"/apps/TBD.AppImage\" process --enqueue %F"
    );
}

#[test]
fn a_path_with_spaces_stays_one_quoted_argument() {
    let text = menu_text(
        Path::new("/home/sam/My Apps/TBD subtitles.AppImage"),
        Path::new("/icons/x.png"),
    );
    assert_eq!(
        exec_line(&text),
        "Exec=\"/home/sam/My Apps/TBD subtitles.AppImage\" process --enqueue %F"
    );
}

#[test]
fn reserved_characters_are_escaped_inside_the_quotes_then_as_a_string() {
    let text = menu_text(
        Path::new("/a \"b\" `c` $d \\e 50%.AppImage"),
        Path::new("/icons/back\\slash.png"),
    );
    assert_eq!(
        exec_line(&text),
        "Exec=\"/a \\\\\"b\\\\\" \\\\`c\\\\` \\\\$d \\\\\\\\e 50%%.AppImage\" process --enqueue %F"
    );
    assert!(text.lines().any(|l| l == "Icon=/icons/back\\\\slash.png"));
}

#[test]
fn install_writes_an_executable_menu_and_the_icon() {
    let root = scratch("install");
    let (menus, icons) = (root.join("menus"), root.join("icons"));
    let source = root.join("source.png");
    fs::write(&source, b"png bytes").unwrap();
    let installed = install_into(&menus, Path::new("/a.AppImage"), Some(&source), &icons).unwrap();
    let menu = menus.join(MENU_FILE);
    assert_eq!(installed, Installed::Written(menu.clone()));
    assert_eq!(
        fs::metadata(&menu).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert_eq!(fs::read(icons.join(ICON_FILE)).unwrap(), b"png bytes");
    let text = fs::read_to_string(&menu).unwrap();
    let icon_line = format!("Icon={}", icons.join(ICON_FILE).display());
    assert!(text.lines().any(|l| l == icon_line));
    assert!(!menus.join(format!("{MENU_FILE}.part")).exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_unchanged_menu_is_not_rewritten_and_a_moved_appimage_is() {
    let root = scratch("unchanged");
    let (menus, icons) = (root.join("menus"), root.join("icons"));
    let menu = menus.join(MENU_FILE);
    install_into(&menus, Path::new("/a.AppImage"), None, &icons).unwrap();
    let written = fs::metadata(&menu).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert_eq!(
        install_into(&menus, Path::new("/a.AppImage"), None, &icons).unwrap(),
        Installed::Unchanged(menu.clone())
    );
    assert_eq!(fs::metadata(&menu).unwrap().modified().unwrap(), written);
    assert_eq!(
        install_into(&menus, Path::new("/b.AppImage"), None, &icons).unwrap(),
        Installed::Written(menu.clone())
    );
    assert!(
        fs::read_to_string(&menu)
            .unwrap()
            .contains("Exec=\"/b.AppImage\" process")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn an_unchanged_menu_gets_its_mode_back() {
    let root = scratch("mode");
    let (menus, icons) = (root.join("menus"), root.join("icons"));
    let menu = menus.join(MENU_FILE);
    install_into(&menus, Path::new("/a.AppImage"), None, &icons).unwrap();
    fs::set_permissions(&menu, fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        install_into(&menus, Path::new("/a.AppImage"), None, &icons).unwrap(),
        Installed::Unchanged(menu.clone())
    );
    assert_eq!(
        fs::metadata(&menu).unwrap().permissions().mode() & 0o777,
        0o755
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn nothing_is_installed_without_an_existing_appimage() {
    assert!(install_from(None, Some(PathBuf::from("/nowhere"))).is_none());
    assert!(install_from(Some(PathBuf::from("/nowhere/tbd.AppImage")), None).is_none());
    if std::env::var_os("APPIMAGE").is_none() {
        assert!(install_from_env().is_none());
    }
}

#[test]
fn the_menu_folder_follows_the_data_home() {
    assert_eq!(
        menu_dir_under(Some("/data".into()), Some("/home/sam".into())),
        Some(PathBuf::from("/data/kio/servicemenus"))
    );
    assert_eq!(
        menu_dir_under(Some("relative".into()), Some("/home/sam".into())),
        Some(PathBuf::from("/home/sam/.local/share/kio/servicemenus"))
    );
    assert_eq!(menu_dir_under(None, None), None);
    assert!(menu_dir().is_none_or(|dir| dir.ends_with("kio/servicemenus")));
}
