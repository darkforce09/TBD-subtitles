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
    let installed = install(
        &video,
        OutputFormat::Srt,
        "1\n",
        &dir.join("backup"),
        "1",
        None,
    )
    .expect("install");
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
    let installed =
        install(&video, OutputFormat::Srt, "new", &backups, "20260926", None).expect("install");
    assert_eq!(installed.backup, Some(backups.join("a.srt.20260926")));
    assert_eq!(
        fs::read_to_string(backups.join("a.srt.20260926")).expect("backup"),
        "old"
    );
    assert_eq!(fs::read_to_string(dir.join("a.srt")).expect("new"), "new");
    let again = install(&video, OutputFormat::Srt, "new", &backups, "later", None).expect("again");
    assert!(again.unchanged);
    assert!(!backups.join("a.srt.later").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_new_format_moves_the_jobs_old_file_aside() {
    let dir = scratch("format");
    let video = dir.join("a.mp4");
    fs::write(dir.join("a.srt"), "old").expect("old");
    let backups = dir.join("backup");
    let installed = install(
        &video,
        OutputFormat::Ass,
        "[Script Info]\n",
        &backups,
        "7",
        Some(&dir.join("a.srt")),
    )
    .expect("install");
    assert_eq!(installed.path, dir.join("a.ass"));
    assert_eq!(installed.retired, Some(backups.join("a.srt.7")));
    assert!(!dir.join("a.srt").exists());
    assert_eq!(
        fs::read_to_string(backups.join("a.srt.7")).expect("moved"),
        "old"
    );
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_ass_install_keeps_the_video_and_old_srt_in_place_until_retry_succeeds() {
    for obstacle in ["a.ass.part", "a.ass"] {
        let dir = scratch(obstacle);
        let video = dir.join("a.mp4");
        let old = dir.join("a.srt");
        let backups = dir.join("backup");
        let source = b"source video is read only";
        let subtitles = b"1\n00:00:01,000 --> 00:00:02,000\nOriginal dialogue\n";
        fs::write(&video, source).expect("source video");
        fs::write(&old, subtitles).expect("existing SRT");
        fs::create_dir(dir.join(obstacle)).expect("block ASS destination");
        let result = install(
            &video,
            OutputFormat::Ass,
            "[Script Info]\n",
            &backups,
            "failed",
            Some(&old),
        );
        assert!(result.is_err(), "{obstacle} must prevent ASS installation");
        assert_eq!(fs::read(&video).expect("source remains"), source);
        assert_eq!(fs::read(&old).expect("SRT remains playable"), subtitles);
        assert!(!backups.join("a.srt.failed").exists());

        fs::remove_dir(dir.join(obstacle)).expect("unblock destination");
        let installed = install(
            &video,
            OutputFormat::Ass,
            "[Script Info]\n",
            &backups,
            "retry",
            Some(&old),
        )
        .expect("retry installation");
        assert_eq!(
            fs::read(&installed.path).expect("installed ASS"),
            b"[Script Info]\n"
        );
        assert_eq!(installed.retired, Some(backups.join("a.srt.retry")));
        assert_eq!(
            fs::read(installed.retired.expect("SRT backup")).expect("backup"),
            subtitles
        );
        assert!(!old.exists());
        assert_eq!(fs::read(&video).expect("source after success"), source);
        let _ = fs::remove_dir_all(&dir);
    }
}

#[test]
fn an_identical_ass_still_retires_the_previous_srt_without_replacing_the_ass() {
    let dir = scratch("identical-format-change");
    let video = dir.join("a.mp4");
    let old = dir.join("a.srt");
    let target = dir.join("a.ass");
    let backups = dir.join("backup");
    fs::write(&video, b"source").expect("source");
    fs::write(&old, b"old SRT").expect("old SRT");
    fs::write(&target, b"[Script Info]\n").expect("existing ASS");
    fs::create_dir(dir.join("a.ass.part")).expect("prevent unnecessary rewriting");
    let installed = install(
        &video,
        OutputFormat::Ass,
        "[Script Info]\n",
        &backups,
        "same",
        Some(&old),
    )
    .expect("confirm installed ASS");
    assert!(installed.unchanged);
    assert_eq!(installed.backup, None);
    assert_eq!(installed.retired, Some(backups.join("a.srt.same")));
    assert_eq!(fs::read(&target).expect("same ASS"), b"[Script Info]\n");
    assert_eq!(
        fs::read(backups.join("a.srt.same")).expect("retired SRT"),
        b"old SRT"
    );
    assert!(!old.exists());
    assert_eq!(fs::read(&video).expect("source unchanged"), b"source");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn only_the_videos_own_subtitle_files_are_moved_aside() {
    let dir = scratch("guard");
    let video = dir.join("a.mp4");
    fs::write(dir.join("notes.srt"), "keep").expect("other");
    let installed = install(
        &video,
        OutputFormat::Srt,
        "1\n",
        &dir.join("backup"),
        "1",
        Some(&dir.join("notes.srt")),
    )
    .expect("install");
    assert_eq!(installed.retired, None);
    assert!(dir.join("notes.srt").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_video_named_like_its_subtitles_is_refused() {
    let result = install(
        Path::new("/tmp/x.srt"),
        OutputFormat::Srt,
        "",
        Path::new("/tmp"),
        "1",
        None,
    );
    assert!(result.is_err());
}
