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
