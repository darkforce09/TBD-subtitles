use job_model::job::{StepMeasure, StepRecord};
use job_model::onscreen::{LocalizedVideoRecord, TextDocument};
use job_model::outputs::{Corrections, SpeechPlan};
use worker_channel::address::Key;

use super::*;
use crate::work_dir::store::scratch::{Scratch, job_record};

fn record(ns: u128) -> StepRecord {
    StepRecord {
        fingerprint: format!("f{ns}"),
        finished_ns: ns,
        measure: StepMeasure::default(),
    }
}

/// Every step finished with a document, a per-frame row and a correction.
fn finished(scratch: &Scratch) {
    let store = scratch.store();
    for (n, step) in StepName::ALL.into_iter().enumerate() {
        store.put_step_record(step, &record(n as u128)).unwrap();
        for key in keys::output_keys(step) {
            let mut write = store.write().unwrap();
            if step == StepName::LocalizedVideo {
                write.put(Table::Outputs, &key, &written_video()).unwrap();
            } else {
                write.put(Table::Outputs, &key, &0u32).unwrap();
            }
            write.commit().unwrap();
        }
    }
    let mut write = store.write().unwrap();
    for table in [Table::Frames, Table::Readings] {
        let key = Key::Frame {
            occurrence: "t1".into(),
            frame: 4,
        };
        write.put(table, &key, &1u32).unwrap();
    }
    write
        .put(
            Table::Corrections,
            &keys::named(keys::LINE_CORRECTIONS),
            &Corrections::default(),
        )
        .unwrap();
    write.commit().unwrap();
}

/// A localized video this job wrote beside its source.
fn written_video() -> LocalizedVideoRecord {
    LocalizedVideoRecord {
        path: Some("/media/episode.localized.mkv".into()),
        ..LocalizedVideoRecord::default()
    }
}

fn steps_with_records(scratch: &Scratch) -> Vec<StepName> {
    scratch
        .store()
        .read()
        .unwrap()
        .step_records()
        .unwrap()
        .into_keys()
        .collect()
}

fn frame_rows(scratch: &Scratch) -> usize {
    let read = scratch.store().read().unwrap();
    read.keys(Table::Frames).unwrap().len() + read.keys(Table::Readings).unwrap().len()
}

#[test]
fn the_cleared_steps_are_the_named_ones_and_every_step_that_reads_them() {
    use StepName::*;
    assert!(cleared_steps(&[]).is_empty());
    assert_eq!(cleared_steps(&[LocalizedVideo]), vec![LocalizedVideo]);
    assert_eq!(
        cleared_steps(&[TextVerify, Output]),
        vec![TextVerify, Output, LocalizedVideo]
    );
    assert_eq!(cleared_steps(&[ProbeDecode]).len(), StepName::ALL.len() - 1);
}

#[test]
fn a_rerun_clears_its_steps_and_their_dependents_in_one_transaction_and_puts_the_record() {
    let scratch = Scratch::new("rerun-clear");
    finished(&scratch);
    let job = job_record(&scratch.dir);
    let cleared = start(scratch.store(), &job, &[StepName::Cues]).unwrap();
    assert_eq!(cleared, cleared_steps(&[StepName::Cues]));
    let left = steps_with_records(&scratch);
    for step in StepName::ALL {
        assert_eq!(left.contains(&step), !cleared.contains(&step), "{step}");
    }
    let read = scratch.store().read().unwrap();
    for step in StepName::ALL {
        for key in keys::output_keys(step) {
            let stored = read.raw(Table::Outputs, &key).unwrap().is_some();
            let carried = step == StepName::LocalizedVideo;
            assert_eq!(stored, carried || !cleared.contains(&step), "{key:?}");
        }
    }
    assert_eq!(read.job_record().unwrap(), Some(job));
    assert!(
        read.raw(Table::Corrections, &keys::named(keys::LINE_CORRECTIONS))
            .unwrap()
            .is_some(),
        "the owner's corrections stay"
    );
    drop(read);
    assert_eq!(
        frame_rows(&scratch),
        0,
        "the cues are upstream of the masks"
    );
}

#[test]
fn per_frame_rows_stay_when_no_step_that_writes_them_is_cleared() {
    let scratch = Scratch::new("rerun-frames");
    finished(&scratch);
    let job = job_record(&scratch.dir);
    start(scratch.store(), &job, &[StepName::Output]).unwrap();
    assert_eq!(frame_rows(&scratch), 2);
    start(scratch.store(), &job, &[StepName::TextVerify]).unwrap();
    assert_eq!(frame_rows(&scratch), 0);
    start(scratch.store(), &job, &[]).unwrap();
    assert_eq!(
        steps_with_records(&scratch).len(),
        StepName::ALL.len() - 3,
        "no rerun clears nothing"
    );
}

#[test]
fn forgetting_a_step_removes_its_record_and_keeps_its_documents() {
    let scratch = Scratch::new("rerun-forget");
    let store = scratch.store();
    store.put_step_record(StepName::Vad, &record(1)).unwrap();
    store
        .put_output(StepName::Vad, None, &SpeechPlan::default())
        .unwrap();
    store
        .put_output(StepName::TextDetect, None, &TextDocument::default())
        .unwrap();
    forget(store, StepName::Vad).unwrap();
    forget(store, StepName::TextDetect).unwrap();
    let read = store.read().unwrap();
    assert_eq!(read.step_record(StepName::Vad).unwrap(), None);
    assert!(
        read.raw(Table::Outputs, &keys::output_key(StepName::Vad, None))
            .unwrap()
            .is_some()
    );
}

#[test]
fn a_cleared_localized_video_keeps_the_video_it_wrote_as_its_earlier_one() {
    let scratch = Scratch::new("rerun-localized");
    finished(&scratch);
    let job = job_record(&scratch.dir);
    start(scratch.store(), &job, &[StepName::TextMask]).unwrap();
    let key = keys::output_key(StepName::LocalizedVideo, None);
    let kept: Option<LocalizedVideoRecord> = scratch
        .store()
        .read()
        .unwrap()
        .get(Table::Outputs, &key)
        .unwrap();
    assert_eq!(
        kept,
        Some(LocalizedVideoRecord {
            earlier: written_video().path,
            ..LocalizedVideoRecord::default()
        })
    );
    assert!(
        !steps_with_records(&scratch).contains(&StepName::LocalizedVideo),
        "the step itself runs again"
    );
}
