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
        segments: Default::default(),
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

#[test]
fn a_job_without_the_localized_video_keeps_its_claim_on_the_stored_earlier_video() {
    use job_model::job::{StepMeasure, StepRecord};
    use worker_channel::address::Table;

    use crate::work_dir::store::scratch::{Scratch, job_record};

    let scratch = Scratch::new("localized-disabled");
    let mut job_row = job_record(&scratch.dir);
    job_row.settings.onscreen_text.localized_video = false;
    let store = scratch.store();
    store.put_job_record(&job_row).unwrap();
    let earlier = scratch.dir.join("episode.localized.mkv");
    store
        .put_output(StepName::LocalizedVideo, None, &record(Some(&earlier)))
        .unwrap();
    let read = store.read().unwrap();
    let inputs = crate::graph::reads(StepName::LocalizedVideo)
        .into_iter()
        .filter_map(|address| {
            let bytes = read.raw(address.table, &address.key).unwrap()?;
            Some((address, bytes))
        })
        .collect();
    drop(read);
    let job = Job {
        work: scratch.work().clone(),
        record: job_row,
        library: None,
    };
    let mut io = StepIo::on_pipe(inputs, std::io::sink());
    run(&job, &mut io, &|_, _| {}).expect("the worker's reads are enough");
    let mut io = StepIo::in_process(store).unwrap();
    let report = run(&job, &mut io, &|_, _| {}).unwrap();
    assert_eq!(
        report.notes.get("disabled").map(String::as_str),
        Some("true")
    );
    let step_record = StepRecord {
        fingerprint: "f".into(),
        finished_ns: 1,
        measure: StepMeasure::default(),
    };
    io.into_outputs()
        .unwrap()
        .commit(StepName::LocalizedVideo, &step_record)
        .unwrap();
    let stored: LocalizedVideoRecord = store
        .read()
        .unwrap()
        .get(
            Table::Outputs,
            &keys::output_key(StepName::LocalizedVideo, None),
        )
        .unwrap()
        .unwrap();
    assert_eq!(
        stored,
        LocalizedVideoRecord {
            earlier: Some(earlier.to_string_lossy().into_owned()),
            ..LocalizedVideoRecord::default()
        }
    );
}
