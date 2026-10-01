use std::path::PathBuf;
use std::time::Duration;

use job_model::onscreen::{LocalizedVideoRecord, SegmentSummary};

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

fn rendered(segments: SegmentSummary) -> Rendered {
    Rendered {
        frames: 1000,
        encoder: "libx264",
        phases: RenderPhases {
            decode_wait: Duration::from_millis(1200),
            blend: Duration::from_millis(3400),
            encode_wait: Duration::from_millis(5600),
            flush: Duration::from_millis(700),
            plan: Duration::from_millis(800),
            copy: Duration::from_millis(900),
            join: Duration::from_millis(1000),
            verify: Duration::from_millis(1100),
        },
        segments,
    }
}

#[test]
fn the_record_carries_the_encoder_and_the_segment_summary() {
    let segments = SegmentSummary {
        segments_reencoded: 2,
        frames_reencoded: 96,
        frames_copied: 904,
        fallback_reason: None,
    };
    let output = Path::new("/videos/episode.localized.mkv");
    assert_eq!(
        record_of(&rendered(segments.clone()), output, 3),
        LocalizedVideoRecord {
            path: Some("/videos/episode.localized.mkv".into()),
            encoder: "libx264".into(),
            frames: 1000,
            replaced: 3,
            earlier: None,
            segments,
        }
    );
}

#[test]
fn the_notes_count_segments_and_frames_and_name_a_fallback() {
    let segments = SegmentSummary {
        segments_reencoded: 2,
        frames_reencoded: 96,
        frames_copied: 904,
        fallback_reason: None,
    };
    let done = rendered(segments);
    let mut report = TaskReport::default();
    note_render(
        &mut report,
        &record_of(&done, Path::new("/v.localized.mkv"), 1),
        &done.phases,
    );
    let note = |key: &str| report.notes.get(key).map(String::as_str);
    assert_eq!(note("encoder"), Some("libx264"));
    assert_eq!(note("frames"), Some("1000"));
    assert_eq!(note("segments_reencoded"), Some("2"));
    assert_eq!(note("frames_reencoded"), Some("96"));
    assert_eq!(note("frames_copied"), Some("904"));
    assert_eq!(note("fallback_reason"), None);
    assert_eq!(note("decode_wait_s"), Some("1.2"));
    assert_eq!(note("blend_s"), Some("3.4"));
    assert_eq!(note("encode_wait_s"), Some("5.6"));
    assert_eq!(note("flush_s"), Some("0.7"));
    assert_eq!(note("plan_s"), Some("0.8"));
    assert_eq!(note("copy_s"), Some("0.9"));
    assert_eq!(note("join_s"), Some("1.0"));
    assert_eq!(note("verify_s"), Some("1.1"));

    let whole = rendered(SegmentSummary {
        segments_reencoded: 1,
        frames_reencoded: 1000,
        frames_copied: 0,
        fallback_reason: Some("the video is hevc, not H.264".into()),
    });
    let mut report = TaskReport::default();
    note_render(
        &mut report,
        &record_of(&whole, Path::new("/v.localized.mkv"), 1),
        &whole.phases,
    );
    assert_eq!(
        report.notes.get("fallback_reason").map(String::as_str),
        Some("the video is hevc, not H.264")
    );
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
