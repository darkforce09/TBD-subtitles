use super::*;

/// A fresh, empty folder for one test.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-video-files-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("dir");
    }
    std::fs::write(path, bytes).expect("file");
}

#[test]
fn a_folder_stands_for_its_videos_without_subtitles() {
    let dir = folder("direct");
    for name in ["b.mp4", "a.MKV", "c.mp4", "c.srt", "notes.txt"] {
        write(&dir.join(name), b"x");
    }
    write(&dir.join("done").join("d.mp4"), b"x");
    assert_eq!(
        videos_in_folder(&dir),
        [dir.join("a.MKV"), dir.join("b.mp4")],
        "sorted, subtitled and non-video files and subfolders left out"
    );
    assert_eq!(subtitle_file(&dir.join("c.mp4")), Some(dir.join("c.srt")));
    assert_eq!(subtitle_file(&dir.join("b.mp4")), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_video_is_known_by_its_extension_in_any_case() {
    for video in ["a.mkv", "a.MP4", "dir/a.Ts", "a.webm"] {
        assert!(is_video(Path::new(video)), "{video}");
    }
    for other in ["a.srt", "a.mkv.part", "mkv", "a"] {
        assert!(!is_video(Path::new(other)), "{other}");
    }
}

#[test]
fn a_part_file_beside_a_video_means_it_is_still_downloading() {
    let dir = folder("partial");
    for (index, suffix) in PARTIAL_SUFFIXES.iter().enumerate() {
        let video = dir.join(format!("{index}.mkv"));
        write(&video, b"x");
        assert!(!has_partial_sibling(&video));
        write(&dir.join(format!("{index}.mkv{suffix}")), b"x");
        assert!(has_partial_sibling(&video), "{suffix}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_walk_finds_videos_in_every_subfolder_sorted() {
    let dir = folder("recursive");
    write(&dir.join("b.mkv"), b"x");
    write(&dir.join("arc").join("z.mp4"), b"x");
    write(&dir.join("arc").join("deeper").join("a.mkv"), b"x");
    write(&dir.join("arc").join("notes.txt"), b"x");
    assert_eq!(
        videos_under(&dir),
        [
            dir.join("arc").join("deeper").join("a.mkv"),
            dir.join("arc").join("z.mp4"),
            dir.join("b.mkv"),
        ]
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_walk_leaves_out_hidden_empty_downloading_and_subtitled_videos() {
    let dir = folder("skipped");
    write(&dir.join("keep.mkv"), b"x");
    write(&dir.join(".hidden").join("a.mkv"), b"x");
    write(&dir.join(".b.mkv"), b"x");
    write(&dir.join("empty.mkv"), b"");
    write(&dir.join("loading.mkv"), b"x");
    write(&dir.join("loading.mkv.part"), b"x");
    write(&dir.join("done.mkv"), b"x");
    write(&dir.join("done.srt"), b"x");
    assert_eq!(videos_under(&dir), [dir.join("keep.mkv")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_walk_does_not_follow_a_symlinked_folder() {
    let dir = folder("symlink");
    let elsewhere = folder("symlink-target");
    write(&elsewhere.join("a.mkv"), b"x");
    std::os::unix::fs::symlink(&elsewhere, dir.join("link")).expect("symlink");
    write(&dir.join("own.mkv"), b"x");
    assert_eq!(videos_under(&dir), [dir.join("own.mkv")]);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&elsewhere);
}

#[test]
fn the_walk_stops_max_depth_folders_down() {
    let dir = folder("depth");
    let mut deepest = dir.clone();
    for level in 0..=MAX_DEPTH {
        deepest = deepest.join(format!("{level}"));
    }
    let at_limit = deepest.parent().expect("parent").join("in.mkv");
    write(&at_limit, b"x");
    write(&deepest.join("out.mkv"), b"x");
    assert_eq!(videos_under(&dir), [at_limit]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_folder_that_cannot_be_read_gives_no_videos() {
    let dir = folder("missing");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(videos_under(&dir).is_empty());
    assert!(videos_in_folder(&dir).is_empty());
}

#[test]
fn a_localized_copy_is_never_taken_as_a_video_to_subtitle() {
    let dir = folder("localized");
    for name in ["a.mkv", "a.localized.mkv", "B.Localized.MKV"] {
        write(&dir.join(name), b"x");
    }
    write(&dir.join("sub").join("c.localized.mkv"), b"x");
    assert!(is_localized_copy(&dir.join("B.Localized.MKV")));
    assert!(!is_localized_copy(&dir.join("a.mkv")));
    assert_eq!(videos_in_folder(&dir), [dir.join("a.mkv")]);
    assert_eq!(videos_under(&dir), [dir.join("a.mkv")]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_job_s_localized_video_is_the_recorded_file_while_it_is_there() {
    let dir = folder("localized-record");
    let work = dir.join("work");
    assert_eq!(localized_video(&work), None, "no record");
    let video = dir.join("a.localized.mkv");
    let record = |path: Option<&Path>| LocalizedVideoRecord {
        path: path.map(|path| path.display().to_string()),
        frames: 10,
        replaced: 2,
        ..LocalizedVideoRecord::default()
    };
    let store = pipeline::work_dir::JobStore::open(&pipeline::work_dir::WorkDir::new(&work))
        .expect("the store");
    let put = |record: &LocalizedVideoRecord| {
        store
            .put_output(StepName::LocalizedVideo, None, record)
            .expect("record");
    };
    put(&record(None));
    assert_eq!(localized_video(&work), None, "a job that wrote none");
    put(&record(Some(&video)));
    assert_eq!(localized_video(&work), None, "the file is gone");
    write(&video, b"x");
    assert_eq!(localized_video(&work), Some(video));
    let _ = std::fs::remove_dir_all(&dir);
}
