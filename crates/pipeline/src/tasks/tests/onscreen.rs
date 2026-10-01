//! The visual text tasks on store fixtures: what they read from `job.redb`, what they store, and
//! that the inputs `graph::reads` sends a worker are enough. No model loads and no process
//! starts.

use std::collections::BTreeMap;

use job_model::job::{JobRecord, StepMeasure, StepRecord};
use job_model::onscreen::{TextCorrections, TextEdit};
use job_model::outputs::{AudioStream, ProbeResult};
use worker_channel::address::Table;

use super::*;
use crate::graph;
use crate::work_dir::store::scratch::{Scratch, job_record};

fn probe() -> ProbeDecoded {
    let track = AudioStream {
        index: 1,
        audio_position: 0,
        codec: "aac".into(),
        language: None,
        channels: 2,
        sample_rate: 48_000,
        start_time_s: 0.0,
    };
    ProbeDecoded {
        probe: ProbeResult {
            duration_s: 20.0,
            video: None,
            audio: vec![track.clone()],
        },
        track,
        samples: 320_000,
    }
}

fn record() -> StepRecord {
    StepRecord {
        fingerprint: "f".into(),
        finished_ns: 1,
        measure: StepMeasure::default(),
    }
}

/// A job whose probe and shot scan are stored, with on-screen text on or off.
fn job(name: &str, enabled: bool) -> (Scratch, Job) {
    let scratch = Scratch::new(name);
    let mut record: JobRecord = job_record(&scratch.dir);
    record.settings.onscreen_text.enabled = enabled;
    let store = scratch.store();
    store.put_job_record(&record).unwrap();
    store
        .put_output(StepName::ProbeDecode, None, &probe())
        .unwrap();
    store
        .put_output(StepName::ShotScan, None, &ShotChanges::default())
        .unwrap();
    let job = Job {
        work: scratch.work().clone(),
        record,
    };
    (scratch, job)
}

/// The step's inputs as a worker receives them: exactly the rows `graph::reads` names.
fn as_worker(scratch: &Scratch, step: StepName) -> StepIo {
    let read = scratch.store().read().unwrap();
    let inputs = graph::reads(step)
        .into_iter()
        .filter_map(|address| {
            let bytes = read.raw(address.table, &address.key).unwrap()?;
            Some((address, bytes))
        })
        .collect();
    StepIo::on_pipe(inputs, std::io::sink())
}

/// Run `step` in process and commit its outputs with its record.
fn run_committed(scratch: &Scratch, job: &Job, step: StepName) -> TaskReport {
    let mut io = StepIo::in_process(scratch.store()).unwrap();
    let report = run(step, job, &mut io, &|_, _| {}).unwrap();
    io.into_outputs().unwrap().commit(step, &record()).unwrap();
    report
}

fn stored<T>(scratch: &Scratch, step: StepName, part: Option<&str>) -> Option<T>
where
    T: rkyv::Archive,
    T::Archived: for<'a> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'a, rkyv::rancor::Error>>
        + rkyv::Deserialize<T, rkyv::api::high::HighDeserializer<rkyv::rancor::Error>>,
{
    scratch
        .store()
        .read()
        .unwrap()
        .get(Table::Outputs, &keys::output_key(step, part))
        .unwrap()
}

#[test]
fn a_job_without_on_screen_text_stores_empty_documents_and_empty_events() {
    let (scratch, job) = job("onscreen-disabled", false);
    for step in [StepName::TextDetect, StepName::TextTypeset] {
        let report = run_committed(&scratch, &job, step);
        assert_eq!(
            report.notes.get("disabled").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            stored::<TextDocument>(&scratch, step, None),
            Some(TextDocument::default())
        );
    }
    assert_eq!(
        stored::<String>(&scratch, StepName::TextTypeset, Some(keys::TYPESET_ASS)),
        Some(String::new())
    );
    assert!(!scratch.dir.join("visual/events.ass").exists());
    assert!(!scratch.dir.join("visual/text_detect.json").exists());
}

#[test]
fn the_review_reads_the_translation_the_probe_the_shots_and_the_text_corrections() {
    let (scratch, job) = job("onscreen-review", true);
    let translated = TextDocument {
        width: 1920,
        height: 1080,
        ..TextDocument::default()
    };
    let store = scratch.store();
    store
        .put_output(StepName::TextTranslate, None, &translated)
        .unwrap();
    let corrections = TextCorrections {
        edits: BTreeMap::from([(
            "text-gone".to_string(),
            TextEdit {
                english: Some("Sea".into()),
                start_s: 1.0,
                end_s: 2.0,
                presentation: Default::default(),
                source_fingerprint: None,
            },
        )]),
        retry: Vec::new(),
    };
    let mut write = store.write().unwrap();
    write
        .put(
            Table::Corrections,
            &keys::named(keys::TEXT_CORRECTIONS),
            &corrections,
        )
        .unwrap();
    write.commit().unwrap();
    let mut io = as_worker(&scratch, StepName::TextReview);
    run(StepName::TextReview, &job, &mut io, &|_, _| {}).expect("the reads are enough");
    run_committed(&scratch, &job, StepName::TextReview);
    let reviewed: TextDocument = stored(&scratch, StepName::TextReview, None).unwrap();
    assert_eq!((reviewed.width, reviewed.height), (1920, 1080));
    assert_eq!(reviewed.review_warnings.len(), 1, "{reviewed:?}");
    assert!(reviewed.review_warnings[0].contains("text-gone"));
}

#[test]
fn typesetting_stores_its_document_and_its_events_from_the_review() {
    let (scratch, job) = job("onscreen-typeset", true);
    scratch
        .store()
        .put_output(StepName::TextReview, None, &TextDocument::default())
        .unwrap();
    let mut io = as_worker(&scratch, StepName::TextTypeset);
    run(StepName::TextTypeset, &job, &mut io, &|_, _| {}).expect("the reads are enough");
    run_committed(&scratch, &job, StepName::TextTypeset);
    assert_eq!(
        stored::<TextDocument>(&scratch, StepName::TextTypeset, None),
        Some(TextDocument::default())
    );
    assert_eq!(
        stored::<String>(&scratch, StepName::TextTypeset, Some(keys::TYPESET_ASS)),
        Some(String::new()),
        "nothing to typeset"
    );
}

#[test]
fn a_step_whose_upstream_document_is_missing_fails_naming_it() {
    let (scratch, job) = job("onscreen-missing", true);
    let mut io = StepIo::in_process(scratch.store()).unwrap();
    let error = run(StepName::TextTypeset, &job, &mut io, &|_, _| {}).unwrap_err();
    assert!(error.to_string().contains("outputs text_review"), "{error}");
}
