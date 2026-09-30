use std::path::PathBuf;

use job_model::onscreen::LocalizedVideoRecord;

use super::*;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-localized-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn record(path: Option<&Path>) -> LocalizedVideoRecord {
    LocalizedVideoRecord {
        path: path.map(|path| path.to_string_lossy().into_owned()),
        encoder: "libx264".into(),
        frames: 10,
        replaced: 1,
        earlier: None,
    }
}

#[test]
fn a_missing_output_may_be_written() {
    let dir = folder("missing");
    let video = dir.join("episode.mkv");
    let output = stages::output::localized_video_path(&video);
    assert!(check_output(&video, &output, None).is_ok());
    assert!(check_output(&video, &output, Some(&record(None))).is_ok());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_foreign_file_at_the_output_path_is_refused() {
    let dir = folder("foreign");
    let video = dir.join("episode.mkv");
    let output = stages::output::localized_video_path(&video);
    std::fs::write(&output, b"someone else's video").unwrap();
    for previous in [
        None,
        Some(record(None)),
        Some(record(Some(&dir.join("other.localized.mkv")))),
    ] {
        let error = check_output(&video, &output, previous.as_ref()).unwrap_err();
        assert_eq!(
            error.message,
            "episode.localized.mkv already exists and was not written by this job; move it away \
             to write the localized video"
        );
    }
    assert_eq!(
        std::fs::read(&output).unwrap(),
        b"someone else's video",
        "the file is left alone"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_jobs_own_earlier_video_is_replaced() {
    let dir = folder("own");
    let video = dir.join("episode.mkv");
    let output = stages::output::localized_video_path(&video);
    std::fs::write(&output, b"an earlier run").unwrap();
    assert!(check_output(&video, &output, Some(&record(Some(&output)))).is_ok());
    let disabled_since = LocalizedVideoRecord {
        earlier: Some(output.to_string_lossy().into_owned()),
        ..LocalizedVideoRecord::default()
    };
    assert!(
        check_output(&video, &output, Some(&disabled_since)).is_ok(),
        "a run without the localized video keeps the job's claim on the file it left"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_source_video_is_never_the_output() {
    let video = PathBuf::from("/videos/episode.localized.mkv");
    assert!(check_output(&video, &video, None).is_err());
}

#[test]
fn the_part_file_sits_beside_the_output() {
    assert_eq!(
        part_path(Path::new("/videos/episode.localized.mkv")),
        PathBuf::from("/videos/episode.localized.mkv.part")
    );
}
