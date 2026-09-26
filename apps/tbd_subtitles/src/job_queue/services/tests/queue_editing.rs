use super::*;

#[test]
fn adding_skips_duplicates_and_empty_paths_and_keeps_order() {
    let mut queue = vec![PathBuf::from("a.mp4")];
    let added = add_videos(
        &mut queue,
        ["b.mp4", "a.mp4", "", "c.mp4", "b.mp4"].map(PathBuf::from),
    );
    assert_eq!(added, 2);
    assert_eq!(queue, ["a.mp4", "b.mp4", "c.mp4"].map(PathBuf::from));
}

#[test]
fn removing_past_the_end_changes_nothing() {
    let mut queue = vec![PathBuf::from("a.mp4"), PathBuf::from("b.mp4")];
    assert_eq!(remove_video(&mut queue, 5), None);
    assert_eq!(remove_video(&mut queue, 0), Some(PathBuf::from("a.mp4")));
    assert_eq!(queue, [PathBuf::from("b.mp4")]);
}

#[test]
fn a_folder_stands_for_its_videos_without_subtitles() {
    let dir = std::env::temp_dir().join(format!("tbd-queue-folder-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("done")).expect("dir");
    for name in ["b.mp4", "a.MKV", "c.mp4", "c.srt", "notes.txt"] {
        std::fs::write(dir.join(name), b"x").expect("file");
    }
    std::fs::write(dir.join("done").join("d.mp4"), b"x").expect("file");
    assert_eq!(
        videos_in_folder(&dir),
        [dir.join("a.MKV"), dir.join("b.mp4")],
        "sorted, subtitled and non-video files and subfolders left out"
    );
    let mut queue = vec![dir.join("b.mp4")];
    assert_eq!(add_videos(&mut queue, [dir.clone()]), 1);
    assert_eq!(queue, [dir.join("b.mp4"), dir.join("a.MKV")]);
    let _ = std::fs::remove_dir_all(&dir);
}
