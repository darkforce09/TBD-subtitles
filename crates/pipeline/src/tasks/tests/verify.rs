//! The replacement and read-back check tasks on store fixtures: what they store when the job
//! writes no localized video, and a check with nothing baked passing the composition on from the
//! inputs `graph::reads` sends its worker. No model loads and no process starts.

use job_model::job::{JobRecord, StepMeasure, StepRecord};
use job_model::onscreen::{ReplacedText, ReplacementDocument};
use worker_channel::address::Table;

use super::*;
use crate::graph;
use crate::work_dir::store::keys;
use crate::work_dir::store::scratch::{Scratch, job_record};

fn job(name: &str, localized_video: bool) -> (Scratch, Job) {
    let scratch = Scratch::new(name);
    let mut record: JobRecord = job_record(&scratch.dir);
    record.settings.onscreen_text.enabled = true;
    record.settings.onscreen_text.localized_video = localized_video;
    scratch.store().put_job_record(&record).unwrap();
    let job = Job {
        work: scratch.work().clone(),
        record,
        library: None,
    };
    (scratch, job)
}

fn commit(io: StepIo, step: StepName) {
    let record = StepRecord {
        fingerprint: "f".into(),
        finished_ns: 1,
        measure: StepMeasure::default(),
    };
    io.into_outputs().unwrap().commit(step, &record).unwrap();
}

fn stored<T>(scratch: &Scratch, step: StepName) -> Option<T>
where
    T: rkyv::Archive,
    T::Archived: for<'a> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'a, rkyv::rancor::Error>>
        + rkyv::Deserialize<T, rkyv::api::high::HighDeserializer<rkyv::rancor::Error>>,
{
    scratch
        .store()
        .read()
        .unwrap()
        .get(Table::Outputs, &keys::output_key(step, None))
        .unwrap()
}

#[test]
fn a_job_without_the_localized_video_stores_empty_replacements_and_checks() {
    let (scratch, job) = job("verify-disabled", false);
    for step in [
        StepName::TextMask,
        StepName::TextInpaint,
        StepName::TextCompose,
    ] {
        let mut io = StepIo::in_process(scratch.store()).unwrap();
        crate::tasks::replace::run(step, &job, &mut io, &|_, _| {}).unwrap();
        commit(io, step);
        assert_eq!(
            stored::<ReplacementDocument>(&scratch, step),
            Some(ReplacementDocument::default())
        );
    }
    let mut io = StepIo::in_process(scratch.store()).unwrap();
    let report = run(&job, &mut io, &|_, _| {}).unwrap();
    assert_eq!(
        report.notes.get("disabled").map(String::as_str),
        Some("true")
    );
    commit(io, StepName::TextVerify);
    assert_eq!(
        stored::<VerifiedReplacements>(&scratch, StepName::TextVerify),
        Some(VerifiedReplacements::default())
    );
    assert!(!scratch.dir.join("visual/text_verify.json").exists());
}

#[test]
fn a_composition_with_nothing_baked_passes_through_the_check_unchanged() {
    let (scratch, job) = job("verify-nothing-baked", true);
    let composed = ReplacementDocument {
        width: 8,
        height: 8,
        frame_count: 10,
        texts: vec![ReplacedText {
            id: "t1".into(),
            first_frame: 0,
            last_frame: 3,
            status: ReplaceStatus::Fallback("the surface moves".into()),
            style: None,
            container: None,
            plates: Vec::new(),
            preview: None,
            lettering_quad: None,
        }],
    };
    scratch
        .store()
        .put_output(StepName::TextCompose, None, &composed)
        .unwrap();
    let read = scratch.store().read().unwrap();
    let inputs = graph::reads(StepName::TextVerify)
        .into_iter()
        .filter_map(|address| {
            let bytes = read.raw(address.table, &address.key).unwrap()?;
            Some((address, bytes))
        })
        .collect();
    drop(read);
    let mut io = StepIo::on_pipe(inputs, std::io::sink());
    let report = run(&job, &mut io, &|_, _| {}).expect("no model and no review are needed");
    assert_eq!(report.notes.get("baked").map(String::as_str), Some("0"));
    let mut io = StepIo::in_process(scratch.store()).unwrap();
    run(&job, &mut io, &|_, _| {}).unwrap();
    commit(io, StepName::TextVerify);
    assert_eq!(
        stored::<VerifiedReplacements>(&scratch, StepName::TextVerify),
        Some(VerifiedReplacements {
            document: composed,
            checks: Vec::new(),
        })
    );
}
