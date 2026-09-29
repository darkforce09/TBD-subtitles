use std::path::Path;
use std::time::{Duration, SystemTime};

use super::*;

fn sample(size: u64) -> Sample {
    Sample {
        size,
        modified: SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
    }
}

fn seen(entries: &[(&str, u64)]) -> Vec<(PathBuf, Sample)> {
    entries
        .iter()
        .map(|(path, size)| (PathBuf::from(path), sample(*size)))
        .collect()
}

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn a_video_is_not_reported_on_its_first_sighting() {
    let mut scan = WatchScan::default();
    assert!(step(&mut scan, seen(&[("a.mkv", 10)])).is_empty());
}

#[test]
fn a_stable_video_is_reported_once_sorted() {
    let mut scan = WatchScan::default();
    step(&mut scan, seen(&[("b.mkv", 10), ("a.mkv", 5)]));
    assert_eq!(
        step(&mut scan, seen(&[("b.mkv", 10), ("a.mkv", 5)])),
        paths(&["a.mkv", "b.mkv"])
    );
    assert!(
        step(&mut scan, seen(&[("b.mkv", 10), ("a.mkv", 5)])).is_empty(),
        "reported once only"
    );
}

#[test]
fn a_growing_video_waits_until_it_stops_changing() {
    let mut scan = WatchScan::default();
    assert!(step(&mut scan, seen(&[("a.mkv", 10)])).is_empty());
    assert!(step(&mut scan, seen(&[("a.mkv", 20)])).is_empty());
    assert!(step(&mut scan, seen(&[("a.mkv", 30)])).is_empty());
    assert_eq!(step(&mut scan, seen(&[("a.mkv", 30)])), paths(&["a.mkv"]));
}

#[test]
fn a_changed_modification_time_is_a_change() {
    let mut scan = WatchScan::default();
    step(&mut scan, seen(&[("a.mkv", 10)]));
    let later = Sample {
        size: 10,
        modified: SystemTime::UNIX_EPOCH + Duration::from_secs(2_000),
    };
    assert!(step(&mut scan, vec![(PathBuf::from("a.mkv"), later)]).is_empty());
}

#[test]
fn samples_of_vanished_videos_are_dropped() {
    let mut scan = WatchScan::default();
    step(&mut scan, seen(&[("a.mkv", 10)]));
    step(&mut scan, Vec::new());
    assert!(scan.samples.is_empty());
    assert!(
        step(&mut scan, seen(&[("a.mkv", 10)])).is_empty(),
        "back again, it is a first sighting"
    );
    assert_eq!(step(&mut scan, seen(&[("a.mkv", 10)])), paths(&["a.mkv"]));
}

#[test]
fn a_reported_video_that_vanishes_and_comes_back_is_not_reported_again() {
    let mut scan = WatchScan::default();
    step(&mut scan, seen(&[("a.mkv", 10)]));
    assert_eq!(step(&mut scan, seen(&[("a.mkv", 10)])), paths(&["a.mkv"]));
    step(&mut scan, Vec::new());
    step(&mut scan, seen(&[("a.mkv", 10)]));
    assert!(step(&mut scan, seen(&[("a.mkv", 10)])).is_empty());
}

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-watch-scan-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    dir
}

fn write(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(path, b"video").expect("file");
}

#[test]
fn scanning_a_folder_reports_its_videos_on_the_second_scan() {
    let dir = folder("scan");
    write(&dir.join("arc").join("b.mkv"));
    write(&dir.join("a.mp4"));
    write(&dir.join("done.mkv"));
    write(&dir.join("done.srt"));
    let mut scan = WatchScan::default();
    let folders = [dir.clone()];
    assert!(scan.scan(&folders).is_empty());
    assert_eq!(
        scan.scan(&folders),
        [dir.join("a.mp4"), dir.join("arc").join("b.mkv")]
    );
    assert!(scan.scan(&folders).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_folder_is_remembered_until_it_reappears() {
    let dir = folder("missing");
    let _ = std::fs::remove_dir_all(&dir);
    let mut scan = WatchScan::default();
    let folders = [dir.clone()];
    assert!(scan.scan(&folders).is_empty());
    assert!(scan.missing.contains(&dir));
    std::fs::create_dir_all(&dir).expect("dir");
    write(&dir.join("a.mkv"));
    assert!(scan.scan(&folders).is_empty());
    assert!(scan.missing.is_empty());
    assert_eq!(scan.scan(&folders), [dir.join("a.mkv")]);
    let _ = std::fs::remove_dir_all(&dir);
}
