use job_model::StepName;
use job_model::job::{StepMeasure, StepRecord};
use job_model::outputs::SpeechPlan;
use worker_channel::address::Table;

use super::scratch::{Scratch, job_record};
use super::*;

fn step(ns: u128) -> StepRecord {
    StepRecord {
        fingerprint: format!("f{ns}"),
        finished_ns: ns,
        measure: StepMeasure::default(),
    }
}

#[test]
fn the_job_record_and_the_step_records_read_back_as_they_were_put() {
    let scratch = Scratch::new("job-rows");
    let store = scratch.store();
    assert_eq!(load_job_record(store).unwrap(), None);
    assert!(load_step_records(store).unwrap().is_empty());
    let record = job_record(&scratch.dir);
    store.put_job_record(&record).unwrap();
    store.put_step_record(StepName::Vad, &step(3)).unwrap();
    store
        .put_step_record(StepName::ProbeDecode, &step(1))
        .unwrap();
    store
        .put_output(StepName::Vad, None, &SpeechPlan::default())
        .unwrap();
    assert_eq!(load_job_record(store).unwrap(), Some(record));
    let steps = load_step_records(store).unwrap();
    assert_eq!(
        steps.keys().copied().collect::<Vec<_>>(),
        vec![StepName::ProbeDecode, StepName::Vad]
    );
    assert_eq!(steps[&StepName::Vad], step(3));
    let stored: Option<SpeechPlan> = store
        .read()
        .unwrap()
        .get(Table::Outputs, &output_key(StepName::Vad, None))
        .unwrap();
    assert_eq!(stored, Some(SpeechPlan::default()));
}

#[test]
fn a_reader_without_a_store_reads_the_job_from_its_folder_and_creates_nothing() {
    let dir = std::env::temp_dir().join(format!("tbd-read-job-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(read_job(&dir).unwrap(), None);
    assert!(!dir.exists(), "no folder is created");
    let record = job_record(&dir);
    {
        let store = JobStore::open(&WorkDir::new(dir.clone())).unwrap();
        assert_eq!(read_job(&dir).unwrap(), None, "no job record yet");
        store.put_job_record(&record).unwrap();
        store.put_step_record(StepName::Qc, &step(9)).unwrap();
        // The reader shares this process's handle while it is open.
        let job = read_job(&dir).unwrap().expect("the job");
        assert_eq!(job.record, record);
    }
    let job = read_job(&dir)
        .unwrap()
        .expect("the job, once the runner closed it");
    assert_eq!(job.steps.len(), 1);
    assert_eq!(job.steps[&StepName::Qc], step(9));
    let _ = std::fs::remove_dir_all(&dir);
}
