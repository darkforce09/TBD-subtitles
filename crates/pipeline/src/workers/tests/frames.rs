use std::io::{self, Write as _};
use std::sync::Mutex;

use worker_channel::frame::write_frame;

use super::*;

/// One frame to write: its tag and its payload.
type Sent = (Tag, Vec<u8>);

/// Write `frames` then `tail` on a pipe from a thread of its own, read them as `step`'s worker,
/// and hand back the events heard and the outcome.
fn read_through_pipe(
    step: StepName,
    frames: Vec<Sent>,
    tail: Vec<u8>,
) -> (Vec<Progress>, Result<WorkerReport, String>) {
    let (mut reader, mut writer) = io::pipe().unwrap();
    let writing = std::thread::spawn(move || {
        for (tag, payload) in frames {
            write_frame(&mut writer, tag, &[&payload]).unwrap();
        }
        writer.write_all(&tail).unwrap();
    });
    let heard = Mutex::new(Vec::new());
    let outcome = read_frames(
        step,
        &mut reader,
        &|event| heard.lock().unwrap().push(event),
        None,
    );
    writing.join().unwrap();
    (heard.into_inner().unwrap(), outcome)
}

fn measure() -> WorkerMeasure {
    WorkerMeasure {
        load_s: 1.5,
        process_s: 12.25,
        peak_ram_mib: 900.0,
        peak_child_ram_mib: 0.0,
        notes: [("batches".to_string(), "4".to_string())].into(),
    }
}

fn measure_frame() -> Sent {
    let archive = rkyv::to_bytes::<rkyv::rancor::Error>(&measure()).unwrap();
    (Tag::Measure, archive.to_vec())
}

fn progress_frame(done: u64, total: u64) -> Sent {
    (
        Tag::Progress,
        ChannelProgress { done, total }.encode().to_vec(),
    )
}

#[test]
fn a_progress_frame_becomes_an_advance() {
    let (heard, outcome) = read_through_pipe(
        StepName::AsrParakeet,
        vec![progress_frame(3, 80)],
        Vec::new(),
    );
    assert_eq!(
        heard,
        [Progress::StepAdvanced {
            step: StepName::AsrParakeet,
            done: 3,
            total: 80
        }]
    );
    assert_eq!(outcome, Ok(WorkerReport::default()));
}

#[test]
fn a_model_call_frame_becomes_a_model_call() {
    let call = ModelExchange {
        id: "9-1".into(),
        model: "sonnet".into(),
        system: "rules\nmore rules".into(),
        ..ModelExchange::default()
    };
    let json = serde_json::to_vec(&call).unwrap();
    let (heard, _) = read_through_pipe(
        StepName::Adjudicate,
        vec![(Tag::ModelCall, json)],
        Vec::new(),
    );
    assert_eq!(
        heard,
        [Progress::ModelCall {
            step: StepName::Adjudicate,
            call: Box::new(call)
        }]
    );
}

#[test]
fn an_unreadable_model_call_is_a_short_message() {
    let (heard, outcome) = read_through_pipe(
        StepName::Adjudicate,
        vec![(Tag::ModelCall, b"{\"id\": ".to_vec())],
        Vec::new(),
    );
    assert_eq!(
        heard,
        [Progress::StepMessage {
            step: StepName::Adjudicate,
            text: "a model call that could not be read (7 bytes)".into()
        }]
    );
    assert!(outcome.is_ok());
}

#[test]
fn a_message_frame_becomes_a_step_message() {
    let (heard, outcome) = read_through_pipe(
        StepName::TextTranslate,
        vec![
            (
                Tag::Message,
                b"waiting for GPU memory: 2048 MiB free, 3822 needed".to_vec(),
            ),
            (Tag::Message, vec![b'a', 0xff]),
        ],
        Vec::new(),
    );
    assert_eq!(
        heard,
        [
            Progress::StepMessage {
                step: StepName::TextTranslate,
                text: "waiting for GPU memory: 2048 MiB free, 3822 needed".into()
            },
            Progress::StepMessage {
                step: StepName::TextTranslate,
                text: "a\u{fffd}".into()
            }
        ]
    );
    assert_eq!(outcome, Ok(WorkerReport::default()));
}

#[test]
fn a_failure_that_is_not_text_is_kept() {
    let message = vec![b'n', b'o', 0xff, 0xfe, 0x00, b'\n', b'!'];
    let (_, outcome) = read_through_pipe(StepName::Vad, vec![(Tag::Failed, message)], Vec::new());
    let report = outcome.unwrap();
    assert_eq!(report.failed.as_deref(), Some("no\u{fffd}\u{fffd}\0\n!"));
    assert!(!report.done);
}

#[test]
fn a_measure_then_the_end_is_a_finished_worker() {
    let (heard, outcome) = read_through_pipe(
        StepName::SoundEvents,
        vec![
            progress_frame(1, 1),
            measure_frame(),
            (Tag::Done, Vec::new()),
        ],
        Vec::new(),
    );
    assert_eq!(heard.len(), 1);
    assert_eq!(
        outcome,
        Ok(WorkerReport {
            measure: Some(measure()),
            failed: None,
            done: true,
        })
    );
}

#[test]
fn a_worker_that_never_ends_says_so() {
    let (_, outcome) = read_through_pipe(StepName::SoundEvents, vec![measure_frame()], Vec::new());
    let report = outcome.unwrap();
    assert_eq!(report.measure, Some(measure()));
    assert!(!report.done);
}

#[test]
fn a_frame_after_the_end_breaks_the_protocol() {
    let (_, outcome) = read_through_pipe(
        StepName::SoundEvents,
        vec![
            measure_frame(),
            (Tag::Done, Vec::new()),
            progress_frame(2, 2),
        ],
        Vec::new(),
    );
    let error = outcome.unwrap_err();
    assert!(error.contains("progress frame after its end"), "{error}");
}

#[test]
fn a_stream_cut_inside_a_frame_breaks_the_protocol() {
    let (heard, outcome) = read_through_pipe(
        StepName::AsrParakeet,
        vec![progress_frame(1, 9)],
        vec![Tag::Progress as u8, 16, 0, 0, 0, 1, 2],
    );
    assert_eq!(heard.len(), 1);
    let error = outcome.unwrap_err();
    assert!(error.contains("broke off"), "{error}");
}

#[test]
fn an_unknown_tag_breaks_the_protocol() {
    let (_, outcome) = read_through_pipe(StepName::Vad, Vec::new(), vec![0x41, 0, 0, 0, 0]);
    assert!(outcome.unwrap_err().contains("unknown tag 65"));
}

#[test]
fn an_output_or_a_bad_measure_breaks_the_protocol() {
    let (_, outcome) = read_through_pipe(
        StepName::Vad,
        vec![(Tag::Output, b"x".to_vec())],
        Vec::new(),
    );
    assert!(outcome.unwrap_err().contains("no step stores yet"));
    let (_, outcome) = read_through_pipe(
        StepName::Vad,
        vec![(Tag::Measure, vec![1, 2, 3])],
        Vec::new(),
    );
    assert!(outcome.unwrap_err().contains("measure could not be read"));
    let (_, outcome) = read_through_pipe(StepName::Vad, vec![(Tag::Input, Vec::new())], Vec::new());
    assert!(outcome.unwrap_err().contains("only the runner sends"));
}

fn report(measure: Option<WorkerMeasure>, failed: Option<&str>, done: bool) -> WorkerReport {
    WorkerReport {
        measure,
        failed: failed.map(str::to_string),
        done,
    }
}

#[test]
fn a_worker_finishes_only_with_exit_zero_its_measure_and_its_end() {
    assert_eq!(
        report(Some(measure()), None, true).verdict(0),
        Ok(measure())
    );
    assert_eq!(
        report(Some(measure()), None, true).verdict(3),
        Err("the worker exited 3".to_string())
    );
    assert_eq!(
        report(None, None, true).verdict(0),
        Err("the worker exited 0 without sending its measure".to_string())
    );
    assert_eq!(
        report(Some(measure()), None, false).verdict(0),
        Err("the worker exited 0 without sending its end".to_string())
    );
    assert_eq!(
        report(None, None, false).verdict(0),
        Err("the worker exited 0 without sending its measure or its end".to_string())
    );
    assert_eq!(
        report(Some(measure()), Some("no model"), true).verdict(1),
        Err("no model\nthe worker exited 1".to_string())
    );
}
