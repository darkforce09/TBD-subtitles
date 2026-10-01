//! A task's stored inputs and outputs, in the runner and on a worker's pipes, with the voice
//! activity step as the task: it reads the probe and writes the chunk plan. No process starts.

use std::io;

use job_model::job::{StepMeasure, StepRecord};
use job_model::outputs::{AudioStream, ProbeResult, SpeechPlan};

use super::*;
use crate::graph;
use crate::tasks::{Job, speech};
use crate::work_dir::store::scratch::{Scratch, job_record};
use crate::workers::channel::inputs::send_inputs;
use crate::workers::frames::read_frames;

fn probe() -> ProbeDecoded {
    let track = AudioStream {
        index: 1,
        audio_position: 0,
        codec: "aac".into(),
        language: Some("eng".into()),
        channels: 2,
        sample_rate: 48_000,
        start_time_s: 0.0,
    };
    ProbeDecoded {
        probe: ProbeResult {
            duration_s: 2.0,
            video: None,
            audio: vec![track.clone()],
        },
        track,
        samples: 32_000,
    }
}

fn record() -> StepRecord {
    StepRecord {
        fingerprint: "vad-fingerprint".into(),
        finished_ns: 7,
        measure: StepMeasure::default(),
    }
}

/// A finished probe and separation: the job record, the probe's document and two seconds of a
/// silent vocal stem.
fn probed(name: &str) -> Scratch {
    let scratch = Scratch::new(name);
    let store = scratch.store();
    store.put_job_record(&job_record(&scratch.dir)).unwrap();
    store
        .put_output(StepName::ProbeDecode, None, &probe())
        .unwrap();
    let silence: Vec<u8> = std::iter::repeat_n(0f32.to_le_bytes(), 32_000)
        .flatten()
        .collect();
    std::fs::create_dir_all(scratch.dir.join("audio")).unwrap();
    std::fs::write(scratch.work().vocals(), silence).unwrap();
    scratch
}

fn stored_plan(scratch: &Scratch) -> Option<SpeechPlan> {
    scratch
        .store()
        .read()
        .unwrap()
        .get(Table::Outputs, &keys::output_key(StepName::Vad, None))
        .unwrap()
}

#[test]
fn an_in_process_step_reads_the_store_and_commits_its_output_with_its_record() {
    let scratch = probed("io-in-process");
    let job = Job {
        work: scratch.work().clone(),
        record: job_record(&scratch.dir),
        library: None,
    };
    let mut io = StepIo::in_process(scratch.store()).unwrap();
    crate::tasks::in_process(StepName::Vad, &job, &mut io, &|_, _| {}).unwrap();
    let outputs = io.into_outputs().expect("the step's pending write");
    assert_eq!(outputs.received(), 1);
    assert_eq!(
        stored_plan(&scratch),
        None,
        "nothing shows before the commit"
    );
    outputs.commit(StepName::Vad, &record()).unwrap();
    let read = scratch.store().read().unwrap();
    assert_eq!(read.step_record(StepName::Vad).unwrap(), Some(record()));
    drop(read);
    let plan = stored_plan(&scratch).expect("the plan");
    assert!(plan.chunks.is_empty(), "silence has no speech");
    assert!(!scratch.dir.join("vad.json").exists());
}

#[test]
fn a_worker_reads_its_inputs_from_its_stdin_and_sends_its_output_as_frames() {
    let scratch = probed("io-worker");
    let inputs = graph::reads(StepName::Vad);
    let (mut stdin_reader, stdin_writer) = io::pipe().unwrap();
    let (mut stdout_reader, stdout_writer) = io::pipe().unwrap();
    let sending = send_inputs(scratch.store().clone(), inputs, &[], stdin_writer);
    let dir = scratch.dir.clone();
    let worker = std::thread::spawn(move || {
        let received = worker_channel::worker::read_inputs(&mut stdin_reader).unwrap();
        let mut io = StepIo::on_pipe(received, stdout_writer);
        let job = Job::received(&dir, &io).unwrap();
        assert_eq!(job.record, job_record(&dir));
        speech::vad(&job, &mut io, &|_, _| {}).unwrap();
        assert!(
            io.into_outputs().is_none(),
            "a worker keeps no pending write"
        );
    });
    let mut outputs = StepWrite::new(scratch.store().clone());
    read_frames(
        StepName::Vad,
        &mut stdout_reader,
        &|_| {},
        Some(&mut outputs),
    )
    .unwrap();
    worker.join().unwrap();
    assert_eq!(sending.join().unwrap(), Ok(()));
    assert_eq!(outputs.received(), 1);
    outputs.commit(StepName::Vad, &record()).unwrap();
    assert!(stored_plan(&scratch).is_some());
    let read = scratch.store().read().unwrap();
    assert_eq!(read.step_record(StepName::Vad).unwrap(), Some(record()));
}

#[test]
fn a_missing_input_is_an_error_naming_its_key() {
    let scratch = Scratch::new("io-missing");
    let io = StepIo::in_process(scratch.store()).unwrap();
    let error = io.probe().expect_err("no probe");
    assert!(
        error.to_string().contains("outputs probe_decode"),
        "{error}"
    );
    assert!(
        error.to_string().contains("not in the job database"),
        "{error}"
    );
    let io = StepIo::in_worker(Vec::new());
    let error = io.job_record().expect_err("no record sent");
    assert!(error.to_string().contains("meta job_record"), "{error}");
    assert!(
        error.to_string().contains("not among the inputs"),
        "{error}"
    );
    assert_eq!(io.corrections().unwrap(), Corrections::default());
    assert_eq!(io.text_corrections().unwrap(), TextCorrections::default());
}

#[test]
fn a_task_writes_only_keys_with_a_kind_and_per_frame_rows_only_in_per_frame_tables() {
    let scratch = Scratch::new("io-keys");
    let mut io = StepIo::in_process(scratch.store()).unwrap();
    assert!(io.put(StepName::Separation, None, &0u32).is_err());
    assert!(io.put(StepName::Vad, Some("other"), &0u32).is_err());
    assert!(io.put_frame(Table::Outputs, "t1", 3, &0u32).is_err());
    io.put_frame(Table::Frames, "t1", 3, &7u32).unwrap();
    io.put_frame(Table::Readings, "t1", 3, &8u32).unwrap();
    io.into_outputs()
        .unwrap()
        .commit(StepName::TextMask, &record())
        .unwrap();
    let read = scratch.store().read().unwrap();
    let key = Key::Frame {
        occurrence: "t1".into(),
        frame: 3,
    };
    assert_eq!(read.get::<u32>(Table::Frames, &key).unwrap(), Some(7));
    assert_eq!(read.get::<u32>(Table::Readings, &key).unwrap(), Some(8));
}

/// Frame rows of two occurrences, out of key order, in the store of a job with a record.
fn with_frame_rows(name: &str) -> Scratch {
    use job_model::onscreen::FrameRecord;

    let scratch = Scratch::new(name);
    let store = scratch.store();
    store.put_job_record(&job_record(&scratch.dir)).unwrap();
    let mut write = store.write().unwrap();
    for (occurrence, frame, dx) in [("b", 4, 2.0), ("a", 7, 1.0), ("a", 6, 0.0)] {
        let key = Key::Frame {
            occurrence: occurrence.into(),
            frame,
        };
        let row = FrameRecord {
            shift: [dx, 0.0],
            scale: 1.0,
            ..FrameRecord::default()
        };
        write.put(Table::Frames, &key, &row).unwrap();
    }
    write.commit().unwrap();
    scratch
}

fn shifts(io: &mut StepIo) -> Vec<(String, u64, f64)> {
    let mut seen = Vec::new();
    io.frame_rows::<job_model::onscreen::FrameRecord>(Table::Frames, |occurrence, frame, row| {
        seen.push((occurrence.to_string(), frame, row.shift[0].to_native()));
        Ok(())
    })
    .unwrap();
    seen
}

#[test]
fn a_step_reads_every_frame_row_in_key_order_in_the_runner_and_in_a_worker() {
    let expected = vec![
        ("a".to_string(), 6, 0.0),
        ("a".to_string(), 7, 1.0),
        ("b".to_string(), 4, 2.0),
    ];
    let scratch = with_frame_rows("io-rows-in-process");
    let mut io = StepIo::in_process(scratch.store()).unwrap();
    assert_eq!(shifts(&mut io), expected);

    let scratch = with_frame_rows("io-rows-worker");
    let (stdin_reader, stdin_writer) = io::pipe().unwrap();
    let sending = send_inputs(
        scratch.store().clone(),
        vec![keys::job_record_address()],
        graph::reads_rows(StepName::TextCompose),
        stdin_writer,
    );
    let mut io = StepIo::over_stdin(StepName::TextCompose, stdin_reader).unwrap();
    assert_eq!(io.job_record().unwrap(), job_record(&scratch.dir));
    assert_eq!(shifts(&mut io), expected);
    io.finish_inputs().unwrap();
    assert_eq!(sending.join().unwrap(), Ok(()));
}

#[test]
fn a_worker_that_reads_no_rows_drains_them_so_the_runner_ends_cleanly() {
    let scratch = with_frame_rows("io-rows-drain");
    let (stdin_reader, stdin_writer) = io::pipe().unwrap();
    let sending = send_inputs(
        scratch.store().clone(),
        vec![keys::job_record_address()],
        graph::reads_rows(StepName::LocalizedVideo),
        stdin_writer,
    );
    let mut io = StepIo::over_stdin(StepName::LocalizedVideo, stdin_reader).unwrap();
    io.finish_inputs().unwrap();
    assert_eq!(sending.join().unwrap(), Ok(()));
    assert!(shifts(&mut io).is_empty(), "the drained rows are gone");
}
