//! The worker channel's data path end to end on real pipes: inputs from a job database down a
//! pipe, and outputs from a pipe into one, with a thread playing the worker. No process starts.

use std::collections::BTreeMap;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use job_model::StepName;
use job_model::job::{StepMeasure, StepRecord, WorkerMeasure};
use job_model::outputs::{AudioStream, ProbeDecoded, ProbeResult};
use worker_channel::address::{Address, Key, Table};
use worker_channel::frame::{Tag, write_frame};
use worker_channel::progress::Progress as ChannelProgress;
use worker_channel::worker::read_inputs;

use super::StepWrite;
use super::inputs::send_inputs;
use crate::progress::Progress;
use crate::work_dir::{JobStore, WorkDir};
use crate::workers::frames::read_frames;

/// A job database in a folder of its own, removed when the test ends.
struct Scratch {
    store: Option<Arc<JobStore>>,
    dir: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("tbd-channel-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = JobStore::open(&WorkDir::new(dir.clone())).expect("the store opens");
        Scratch {
            store: Some(store),
            dir,
        }
    }

    fn store(&self) -> &Arc<JobStore> {
        self.store.as_ref().expect("open")
    }

    /// Every key of the tables a step writes: its outputs and its record.
    fn stored(&self) -> (Vec<Key>, Vec<Key>) {
        let read = self.store().read().expect("read");
        (
            read.keys(Table::Outputs).expect("outputs"),
            read.keys(Table::StepRecords).expect("step records"),
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(self.store.take());
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn named(table: Table, name: &str) -> Address {
    Address {
        table,
        key: Key::Name(name.into()),
    }
}

fn archive<T>(value: &T) -> Vec<u8>
where
    T: for<'a> rkyv::Serialize<
            rkyv::api::high::HighSerializer<
                rkyv::util::AlignedVec,
                rkyv::ser::allocator::ArenaHandle<'a>,
                rkyv::rancor::Error,
            >,
        >,
{
    rkyv::to_bytes::<rkyv::rancor::Error>(value)
        .expect("archive")
        .to_vec()
}

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
            duration_s: 1_440.5,
            video: None,
            audio: vec![track.clone()],
        },
        track,
        samples: 23_048_000,
    }
}

fn measure() -> WorkerMeasure {
    WorkerMeasure {
        load_s: 0.5,
        process_s: 2.0,
        peak_ram_mib: 300.0,
        peak_child_ram_mib: 0.0,
        notes: BTreeMap::new(),
    }
}

fn record(measure: &WorkerMeasure) -> StepRecord {
    StepRecord {
        fingerprint: "f00d".into(),
        finished_ns: 42,
        measure: StepMeasure {
            wall_s: 2.5,
            load_s: Some(measure.load_s),
            process_s: Some(measure.process_s),
            peak_ram_mib: Some(measure.peak_ram_mib),
            ..StepMeasure::default()
        },
    }
}

/// One frame's bytes.
fn frame(tag: Tag, parts: &[&[u8]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_frame(&mut bytes, tag, parts).expect("frame");
    bytes
}

fn output(address: &Address, archive: &[u8]) -> Vec<u8> {
    frame(Tag::Output, &[&address.encode().expect("address"), archive])
}

fn measure_frame() -> Vec<u8> {
    frame(Tag::Measure, &[&archive(&measure())])
}

fn done() -> Vec<u8> {
    frame(Tag::Done, &[])
}

fn progress_frame(done: u64, total: u64) -> Vec<u8> {
    frame(Tag::Progress, &[&ChannelProgress { done, total }.encode()])
}

/// The worker's side: `bytes` written to a pipe from a thread of its own, then the pipe closed.
fn worker_writes(bytes: Vec<Vec<u8>>) -> (io::PipeReader, std::thread::JoinHandle<()>) {
    let (reader, mut writer) = io::pipe().expect("pipe");
    let writing = std::thread::spawn(move || {
        for chunk in bytes {
            // The runner stops reading at a protocol break; later writes may find the pipe gone.
            if writer.write_all(&chunk).is_err() {
                return;
            }
        }
    });
    (reader, writing)
}

/// What the runner does with a worker that sent `bytes` and exited `code`: read its frames into a
/// step write, decide, and commit with the step's record only when the step finished. The events
/// heard and the step's outcome.
fn run_step(
    scratch: &Scratch,
    step: StepName,
    bytes: Vec<Vec<u8>>,
    code: i32,
) -> (Vec<Progress>, Result<usize, String>) {
    let (mut reader, writing) = worker_writes(bytes);
    let heard = Mutex::new(Vec::new());
    let mut outputs = StepWrite::new(scratch.store().clone());
    let read = read_frames(
        step,
        &mut reader,
        &|event| heard.lock().unwrap().push(event),
        Some(&mut outputs),
    );
    drop(reader);
    writing.join().expect("the worker thread");
    let outcome = read.and_then(|report| report.verdict(code)).map(|measure| {
        let received = outputs.received();
        outputs.commit(step, &record(&measure)).expect("the commit");
        received
    });
    (heard.into_inner().unwrap(), outcome)
}

#[test]
fn the_runner_sends_stored_inputs_that_the_worker_reads_back_byte_exact() {
    let scratch = Scratch::new("inputs");
    let probe_at = named(Table::Outputs, "probe_decode");
    let record_at = named(Table::StepRecords, "vad");
    let mut write = scratch.store().write().expect("write");
    write
        .put(probe_at.table, &probe_at.key, &probe())
        .expect("put");
    write
        .put(record_at.table, &record_at.key, &record(&measure()))
        .expect("put");
    write.commit().expect("commit");

    let (mut reader, writer) = io::pipe().expect("pipe");
    let sending = send_inputs(
        scratch.store().clone(),
        vec![probe_at.clone(), record_at.clone()],
        writer,
    );
    let inputs = read_inputs(&mut reader).expect("the worker reads its inputs");
    assert_eq!(sending.join().expect("thread"), Ok(()));

    let read = scratch.store().read().expect("read");
    let expected: Vec<(Address, Vec<u8>)> = [probe_at, record_at]
        .into_iter()
        .map(|at| {
            let bytes = read.raw(at.table, &at.key).expect("raw").expect("stored");
            (at, bytes)
        })
        .collect();
    assert_eq!(inputs, expected);
    let back = rkyv::from_bytes::<ProbeDecoded, rkyv::rancor::Error>(&inputs[0].1).expect("read");
    assert_eq!(back, probe());
}

#[test]
fn a_missing_input_is_an_error_and_closes_the_pipe() {
    let scratch = Scratch::new("missing-input");
    let (mut reader, writer) = io::pipe().expect("pipe");
    let sending = send_inputs(
        scratch.store().clone(),
        vec![named(Table::Outputs, "vad")],
        writer,
    );
    assert_eq!(read_inputs(&mut reader).expect("a clean end"), Vec::new());
    let error = sending.join().expect("thread").expect_err("missing");
    assert!(
        error.contains("outputs vad is not in the job database"),
        "{error}"
    );
}

#[test]
fn outputs_arrive_in_place_and_commit_with_the_step_record() {
    let scratch = Scratch::new("outputs");
    let dropped = vec!["[THUD]".to_string(), "[SIGHS]".to_string()];
    let (heard, outcome) = run_step(
        &scratch,
        StepName::ProbeDecode,
        vec![
            output(&named(Table::Outputs, "probe_decode"), &archive(&probe())),
            progress_frame(1, 2),
            output(
                &named(Table::Outputs, "cues/dropped_sounds"),
                &archive(&dropped),
            ),
            measure_frame(),
            done(),
        ],
        0,
    );
    assert_eq!(outcome, Ok(2));
    assert_eq!(
        heard,
        [Progress::StepAdvanced {
            step: StepName::ProbeDecode,
            done: 1,
            total: 2
        }]
    );

    let read = scratch.store().read().expect("a fresh snapshot");
    let samples = read
        .view::<ProbeDecoded, _>(Table::Outputs, &Key::Name("probe_decode".into()), |probe| {
            probe.samples.to_native()
        })
        .expect("view")
        .expect("stored");
    assert_eq!(samples, 23_048_000);
    let first = read
        .view::<Vec<String>, _>(
            Table::Outputs,
            &Key::Name("cues/dropped_sounds".into()),
            |sounds| sounds[0].to_string(),
        )
        .expect("view")
        .expect("stored");
    assert_eq!(first, "[THUD]");
    let stored = read
        .get::<StepRecord>(Table::StepRecords, &Key::Name("probe_decode".into()))
        .expect("get");
    assert_eq!(stored, Some(record(&measure())));
}

#[test]
fn a_step_that_does_not_finish_stores_nothing() {
    let scratch = Scratch::new("unfinished");
    let probe_output = || output(&named(Table::Outputs, "probe_decode"), &archive(&probe()));
    let cases: Vec<(&str, Vec<Vec<u8>>, i32, &str)> = vec![
        (
            "a failed frame",
            vec![
                probe_output(),
                frame(Tag::Failed, &[b"no audio track"]),
                measure_frame(),
            ],
            1,
            "no audio track",
        ),
        (
            "no end",
            vec![probe_output(), measure_frame()],
            0,
            "without sending its end",
        ),
        (
            "a non-zero exit",
            vec![probe_output(), measure_frame(), done()],
            2,
            "the worker exited 2",
        ),
        (
            "an archive that does not check",
            vec![
                probe_output(),
                output(&named(Table::Outputs, "vad"), &[0xff; 40]),
                measure_frame(),
                done(),
            ],
            0,
            "outputs vad was refused",
        ),
        (
            "an unknown output key",
            vec![
                probe_output(),
                output(&named(Table::Outputs, "separation"), &archive(&probe())),
            ],
            0,
            "no record type",
        ),
        (
            "an output to a table a worker never writes",
            vec![output(
                &named(Table::StepRecords, "probe_decode"),
                &archive(&record(&measure())),
            )],
            0,
            "never writes the step_records table",
        ),
        (
            "a frame shorter than its address",
            vec![probe_output(), frame(Tag::Output, &[&[3, 0]])],
            0,
            "address could not be read",
        ),
        (
            "a stream cut inside an output",
            vec![probe_output(), {
                let whole = output(&named(Table::Outputs, "vad"), &[7; 200]);
                whole[..whole.len() - 150].to_vec()
            }],
            0,
            "outputs vad was refused",
        ),
    ];
    for (case, bytes, code, expected) in cases {
        let (_, outcome) = run_step(&scratch, StepName::ProbeDecode, bytes, code);
        let error = outcome.expect_err(case);
        assert!(error.contains(expected), "{case}: {error}");
        assert_eq!(scratch.stored(), (Vec::new(), Vec::new()), "{case}");
    }
}

#[test]
fn an_unknown_output_is_refused_before_the_step_write_begins() {
    let scratch = Scratch::new("unknown-first");
    let mut outputs = StepWrite::new(scratch.store().clone());
    let archived = archive(&probe());
    let error = outputs
        .receive(
            named(Table::Outputs, "not_a_step"),
            archived.len(),
            &mut archived.as_slice(),
        )
        .expect_err("unknown");
    assert!(error.contains("no record type"), "{error}");
    assert!(!outputs.is_open());
    assert_eq!(outputs.received(), 0);
}

#[test]
fn a_step_write_dropped_without_commit_stores_nothing() {
    let scratch = Scratch::new("dropped");
    let mut outputs = StepWrite::new(scratch.store().clone());
    let archived = archive(&probe());
    outputs
        .receive(
            named(Table::Outputs, "probe_decode"),
            archived.len(),
            &mut archived.as_slice(),
        )
        .expect("kept");
    assert!(outputs.is_open());
    drop(outputs);
    assert_eq!(scratch.stored(), (Vec::new(), Vec::new()));
}
