use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-output-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch");
    dir
}

#[test]
fn the_file_lands_beside_the_video_with_its_base_name() {
    let dir = scratch("new");
    let video = dir.join("[Muhn Pace] Dressrosa 11.mp4");
    fs::write(&video, b"video").expect("video");
    let installed = install(&video, "1\n", &dir.join("backup"), "1").expect("install");
    assert_eq!(installed.path, dir.join("[Muhn Pace] Dressrosa 11.srt"));
    assert_eq!(fs::read_to_string(&installed.path).expect("read"), "1\n");
    assert_eq!(installed.backup, None);
    assert_eq!(fs::read(&video).expect("video"), b"video");
    assert!(!dir.join("[Muhn Pace] Dressrosa 11.srt.part").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_different_file_is_backed_up_and_an_identical_one_is_left_alone() {
    let dir = scratch("replace");
    let video = dir.join("a.mp4");
    fs::write(dir.join("a.srt"), "old").expect("old");
    let backups = dir.join("work").join("backup");
    let installed = install(&video, "new", &backups, "20260926").expect("install");
    assert_eq!(installed.backup, Some(backups.join("a.srt.20260926")));
    assert_eq!(
        fs::read_to_string(backups.join("a.srt.20260926")).expect("backup"),
        "old"
    );
    assert_eq!(fs::read_to_string(dir.join("a.srt")).expect("new"), "new");
    let again = install(&video, "new", &backups, "later").expect("again");
    assert!(again.unchanged);
    assert!(!backups.join("a.srt.later").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_video_named_srt_is_refused() {
    assert!(install(Path::new("/tmp/x.srt"), "", Path::new("/tmp"), "1").is_err());
}
