use super::*;

#[test]
fn every_file_below_the_folder_counts_once() {
    let root = std::env::temp_dir().join(format!("tbd-work-size-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("job").join("audio")).expect("dirs");
    std::fs::write(root.join("job").join("job.json"), [0u8; 10]).expect("file");
    std::fs::write(root.join("job").join("audio").join("mix.f32"), [0u8; 100]).expect("file");
    std::os::unix::fs::symlink(root.join("job"), root.join("link")).expect("link");
    assert_eq!(size(&root), 110);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_missing_folder_is_empty() {
    assert_eq!(size(Path::new("/no/such/work")), 0);
}
