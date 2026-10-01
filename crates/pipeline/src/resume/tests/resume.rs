use std::path::PathBuf;

use job_model::job::{StepMeasure, StepRecord};
use job_model::onscreen::{
    LocalizedVideoRecord, ReplacementDocument, TextDocument, TextKeyframe, TextOccurrence,
    VerifiedReplacements,
};
use job_model::outputs::{Chosen, Correction, OutputRecord};
use worker_channel::address::Key;

use super::*;
use crate::work_dir::store::scratch::{Scratch, job_record};
use crate::work_dir::{self, JobStore};

/// Record `step` as finished at `ns` with its current fingerprint.
fn finish(store: &JobStore, record: &JobRecord, step: StepName, ns: u128) {
    let fingerprint = fingerprint(step, record, &store.read().unwrap(), None).unwrap();
    store
        .put_step_record(
            step,
            &StepRecord {
                fingerprint,
                finished_ns: ns,
                measure: StepMeasure::default(),
            },
        )
        .unwrap();
}

fn current(step: StepName, record: &JobRecord, store: &JobStore) -> String {
    fingerprint(step, record, &store.read().unwrap(), None).unwrap()
}

fn put_bytes(store: &JobStore, key: &Key) {
    let mut write = store.write().unwrap();
    write.put(Table::Outputs, key, &0u32).unwrap();
    write.commit().unwrap();
}

/// Every document `step` writes, and the files its rows name, as a finished step leaves them.
fn leave_outputs(scratch: &Scratch, record: &JobRecord, step: StepName) {
    let store = scratch.store();
    match step {
        StepName::ProbeDecode => {
            scratch.file("audio/mix_16k.f32", b"x");
            put_bytes(store, &keys::output_key(step, None));
        }
        StepName::Separation => {
            scratch.file("audio/vocals_16k.f32", b"x");
            scratch.file("audio/background_16k.f32", b"x");
        }
        StepName::TextDetect => {
            scratch.file("visual/crops/one.png", b"x");
            let document = TextDocument {
                occurrences: vec![occurrence("visual/crops/one.png")],
                ..TextDocument::default()
            };
            store.put_output(step, None, &document).unwrap();
        }
        StepName::Output => {
            let subtitles = PathBuf::from(&record.video).with_extension("ass");
            std::fs::write(&subtitles, b"").unwrap();
            let output = OutputRecord {
                path: subtitles.to_string_lossy().into_owned(),
                ..OutputRecord::default()
            };
            store.put_output(step, None, &output).unwrap();
        }
        StepName::LocalizedVideo => {
            store
                .put_output(step, None, &LocalizedVideoRecord::default())
                .unwrap();
        }
        StepName::TextRead
        | StepName::TextTrack
        | StepName::TextTranslate
        | StepName::TextReview => {
            store
                .put_output(step, None, &TextDocument::default())
                .unwrap();
        }
        StepName::TextTypeset => {
            store
                .put_output(step, None, &TextDocument::default())
                .unwrap();
            store
                .put_output(step, Some(keys::TYPESET_ASS), &String::new())
                .unwrap();
        }
        StepName::TextMask | StepName::TextInpaint | StepName::TextCompose => {
            store
                .put_output(step, None, &ReplacementDocument::default())
                .unwrap();
        }
        StepName::TextVerify => {
            store
                .put_output(step, None, &VerifiedReplacements::default())
                .unwrap();
        }
        _ => {
            for key in keys::output_keys(step) {
                put_bytes(store, &key);
            }
        }
    }
}

fn occurrence(crop: &str) -> TextOccurrence {
    TextOccurrence {
        id: "t1".into(),
        start_s: 1.0,
        end_s: 2.0,
        japanese: "海".into(),
        english: None,
        confidence: 0.9,
        crops: vec![crop.into()],
        frames: Vec::new(),
        provenance: Default::default(),
        presentation: Default::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: None::<TextKeyframe>,
        ruby: Vec::new(),
    }
}

/// A job with every step finished in order.
fn finished_job(name: &str) -> (Scratch, JobRecord) {
    let scratch = Scratch::new(name);
    let record = job_record(&scratch.dir);
    scratch.store().put_job_record(&record).unwrap();
    for (index, step) in StepName::ALL.into_iter().enumerate() {
        leave_outputs(&scratch, &record, step);
        finish(scratch.store(), &record, step, index as u128 + 1);
    }
    (scratch, record)
}

fn stale(scratch: &Scratch, record: &JobRecord) -> Vec<StepName> {
    stale_steps(
        record,
        &scratch.store().read().unwrap(),
        scratch.work(),
        None,
    )
}

#[test]
fn a_missing_visual_crop_invalidates_the_visual_steps_without_repeating_audio() {
    let (scratch, record) = finished_job("resume-crop");
    assert!(stale(&scratch, &record).is_empty(), "a finished job");
    std::fs::remove_file(scratch.dir.join("visual/crops/one.png")).unwrap();
    assert_eq!(
        stale(&scratch, &record),
        vec![
            StepName::TextDetect,
            StepName::TextRead,
            StepName::TextTrack,
            StepName::TextTranslate,
            StepName::TextReview,
            StepName::TextMask,
            StepName::TextInpaint,
            StepName::TextCompose,
            StepName::TextVerify,
            StepName::TextTypeset,
            StepName::Qc,
            StepName::Output,
            StepName::LocalizedVideo,
        ]
    );
}

#[test]
fn a_finished_step_with_its_rows_and_files_is_reused_and_a_missing_one_reruns_it() {
    let scratch = Scratch::new("resume-files");
    let store = scratch.store();
    let record = job_record(&scratch.dir);
    let valid = || {
        is_valid(
            StepName::ProbeDecode,
            &record,
            &store.read().unwrap(),
            scratch.work(),
            None,
        )
    };
    finish(store, &record, StepName::ProbeDecode, 1);
    assert!(!valid(), "no document and no file yet");
    let mix = scratch.file("audio/mix_16k.f32", b"x");
    assert!(!valid(), "no document yet");
    put_bytes(store, &keys::output_key(StepName::ProbeDecode, None));
    assert!(valid());
    std::fs::remove_file(&mix).unwrap();
    assert!(!valid());
    // A part file left by a killed run is not an output.
    scratch.file("audio/mix_16k.f32.part", b"x");
    assert!(!valid());
    scratch.file("audio/mix_16k.f32", b"x");
    let mut write = store.write().unwrap();
    write
        .remove(
            Table::Outputs,
            &keys::output_key(StepName::ProbeDecode, None),
        )
        .unwrap();
    write.commit().unwrap();
    assert!(!valid(), "the document went");
}

#[test]
fn a_rerun_upstream_step_invalidates_every_step_that_reads_it() {
    let scratch = Scratch::new("resume-upstream");
    let store = scratch.store();
    let r = job_record(&scratch.dir);
    finish(store, &r, StepName::ProbeDecode, 1);
    finish(store, &r, StepName::Separation, 2);
    finish(store, &r, StepName::Vad, 3);
    let vad = current(StepName::Vad, &r, store);
    let stored = store.read().unwrap().step_record(StepName::Vad).unwrap();
    assert_eq!(vad, stored.unwrap().fingerprint);
    // Separation runs again: same settings, a new finish time.
    finish(store, &r, StepName::Separation, 9);
    assert_ne!(current(StepName::Vad, &r, store), vad);
    // The shot scan reads nothing but the video and is untouched.
    let shots = current(StepName::ShotScan, &r, store);
    finish(store, &r, StepName::ProbeDecode, 10);
    assert_eq!(current(StepName::ShotScan, &r, store), shots);
}

#[test]
fn a_setting_changes_only_the_steps_that_read_it_and_the_video_changes_the_first() {
    let scratch = Scratch::new("resume-settings");
    let store = scratch.store();
    let r = job_record(&scratch.dir);
    let mut tuned = r.clone();
    tuned.settings.cut_score = 35.0;
    assert_ne!(
        current(StepName::Cues, &r, store),
        current(StepName::Cues, &tuned, store)
    );
    assert_eq!(
        current(StepName::Alignment, &r, store),
        current(StepName::Alignment, &tuned, store)
    );
    let mut touched = r.clone();
    touched.video_modified_s = 6;
    assert_ne!(
        current(StepName::ProbeDecode, &r, store),
        current(StepName::ProbeDecode, &touched, store)
    );
    assert_eq!(
        current(StepName::Vad, &r, store),
        current(StepName::Vad, &touched, store)
    );
}

#[test]
fn the_stale_steps_are_the_invalid_ones_and_everything_that_reads_them() {
    let scratch = Scratch::new("resume-new");
    let record = job_record(&scratch.dir);
    assert_eq!(
        stale(&scratch, &record),
        StepName::ALL.to_vec(),
        "a new job"
    );
    let (scratch, record) = finished_job("resume-stale");
    let mut write = scratch.store().write().unwrap();
    write
        .remove(Table::StepRecords, &keys::record_key(StepName::Cues))
        .unwrap();
    write.commit().unwrap();
    assert_eq!(
        stale(&scratch, &record),
        vec![
            StepName::Cues,
            StepName::TextTranslate,
            StepName::TextReview,
            StepName::TextMask,
            StepName::TextInpaint,
            StepName::TextCompose,
            StepName::TextVerify,
            StepName::TextTypeset,
            StepName::Qc,
            StepName::Output,
            StepName::LocalizedVideo
        ]
    );
}

#[test]
fn the_line_corrections_reach_the_review_step_alone() {
    let scratch = Scratch::new("resume-corrections");
    let store = scratch.store();
    let r = job_record(&scratch.dir);
    let before: Vec<String> = StepName::ALL
        .iter()
        .map(|s| current(*s, &r, store))
        .collect();
    work_dir::update_corrections(store, |c| {
        c.set(Correction {
            id: "U0001".into(),
            text: "Line.".into(),
            flags: Vec::new(),
            chosen: Chosen::Typed,
        })
    })
    .unwrap();
    for (step, old) in StepName::ALL.iter().zip(&before) {
        let changed = current(*step, &r, store) != *old;
        assert_eq!(changed, *step == StepName::Review, "{step}");
    }
}

#[test]
fn the_text_corrections_reach_reading_translation_and_review_alone() {
    let scratch = Scratch::new("resume-text-corrections");
    let store = scratch.store();
    let mut r = job_record(&scratch.dir);
    r.settings.onscreen_text.enabled = true;
    let before: Vec<String> = StepName::ALL
        .iter()
        .map(|s| current(*s, &r, store))
        .collect();
    work_dir::update_text_corrections(store, |c| c.retry.push("t1".into())).unwrap();
    for (step, old) in StepName::ALL.iter().zip(&before) {
        let changed = current(*step, &r, store) != *old;
        let reads = matches!(
            step,
            StepName::TextRead | StepName::TextTranslate | StepName::TextReview
        );
        assert_eq!(changed, reads, "{step}");
    }
}

#[test]
fn a_changed_table_layout_changes_every_fingerprint() {
    let scratch = Scratch::new("resume-layout");
    let store = scratch.store();
    let r = job_record(&scratch.dir);
    let before = current(StepName::ShotScan, &r, store);
    let mut layouts = TableLayouts::default();
    layouts.versions.insert("outputs".into(), 99);
    let mut write = store.write().unwrap();
    write
        .put(Table::Meta, &keys::named(keys::LAYOUT), &layouts)
        .unwrap();
    write.commit().unwrap();
    assert_ne!(current(StepName::ShotScan, &r, store), before);
}

#[test]
fn only_a_matched_sign_of_another_job_reaches_the_translation_and_composition_fingerprints() {
    use crate::library::fixtures::{hash, library_sign, occurrence};
    let scratch = Scratch::new("resume-library");
    let store = scratch.store();
    let mut record = job_record(&scratch.dir);
    record.settings.onscreen_text.enabled = true;
    record.settings.onscreen_text.localized_video = true;
    let library = Library::at(scratch.dir.join("library").join(crate::library::FILE_NAME));
    let text = TextDocument {
        occurrences: vec![occurrence(&scratch.dir, "a", "王宮", 1)],
        ..TextDocument::default()
    };
    store.put_output(StepName::TextTrack, None, &text).unwrap();
    store.put_output(StepName::TextReview, None, &text).unwrap();
    let with = |step| fingerprint(step, &record, &store.read().unwrap(), Some(&library)).unwrap();
    let without = |step| current(step, &record, store);
    let steps = [
        StepName::TextTranslate,
        StepName::TextCompose,
        StepName::TextReview,
    ];
    for step in steps {
        assert_eq!(with(step), without(step), "{step}: an empty library");
    }
    let own = work_dir::job_id(Path::new(&record.video));
    library
        .record(library_sign("王宮", hash(1), "Royal Palace", &own))
        .unwrap();
    for step in steps {
        assert_eq!(with(step), without(step), "{step}: the job's own sign");
    }
    library.clear().unwrap();
    library
        .record(library_sign("王宮", hash(1), "Royal Palace", "d11"))
        .unwrap();
    let matched = [with(StepName::TextTranslate), with(StepName::TextCompose)];
    assert_ne!(matched[0], without(StepName::TextTranslate));
    assert_ne!(matched[1], without(StepName::TextCompose));
    assert_eq!(with(StepName::TextReview), without(StepName::TextReview));
    library.remove("王宮", hash(1)).unwrap();
    library
        .record(library_sign("王宮", hash(1), "The Palace", "d11"))
        .unwrap();
    assert_ne!(with(StepName::TextTranslate), matched[0]);
    assert_ne!(with(StepName::TextCompose), matched[1]);
}
