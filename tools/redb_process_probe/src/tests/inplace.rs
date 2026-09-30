use super::*;

#[test]
fn an_archive_round_trips_in_place_from_redb() {
    let dir = std::env::temp_dir().join(format!(
        "redb-process-probe-inplace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ));
    let result = prepare(&dir).and_then(run);
    let file_left = dir.join("inplace.redb").exists();
    let _ = std::fs::remove_dir_all(&dir);
    let lines = result.expect("the in-place check holds");
    assert_eq!(lines.last().map(String::as_str), Some("inplace: ok"));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("redb slice: accessed in place: name=\"text_inpaint\""))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("shifted copy: accessed in place:"))
    );
    assert!(!file_left, "the database file is removed");
}

#[test]
fn an_odd_copy_starts_at_an_odd_address_and_keeps_every_byte() {
    let bytes: Vec<u8> = (0..=40).collect();
    for _ in 0..8 {
        let copy = odd_copy(&bytes);
        assert_eq!((copy.slice().as_ptr() as usize) % 2, 1);
        assert_eq!(copy.slice(), bytes.as_slice());
    }
}

#[test]
fn a_folder_that_cannot_be_created_is_an_error() {
    assert!(
        prepare(std::path::Path::new(
            "/proc/redb-process-probe-cannot-be-here"
        ))
        .is_err()
    );
}
