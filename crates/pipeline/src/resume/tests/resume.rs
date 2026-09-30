use std::collections::BTreeMap;

use job_model::job::{JobSettings, StepMeasure, StepRecord};

use super::*;

fn scratch(name: &str) -> WorkDir {
    let dir = std::env::temp_dir().join(format!("tbd-resume-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch");
    WorkDir::new(dir)
}

fn record() -> JobRecord {
    JobRecord {
        video: "/videos/a.mp4".into(),
        video_size: 10,
        video_modified_s: 5,
        settings: JobSettings::with_glossary(vec!["Luffy".into()]),
        models_dir: None,
        corrections: None,
        steps: BTreeMap::new(),
    }
}

/// Record `step` as finished at `ns` with its current fingerprint.
fn finish(record: &mut JobRecord, step: StepName, ns: u128) {
    let fingerprint = fingerprint(step, record);
    record.steps.insert(
        step,
        StepRecord {
            fingerprint,
            finished_ns: ns,
            measure: StepMeasure::default(),
        },
    );
}

fn touch(paths: &[std::path::PathBuf]) {
    for p in paths {
        fs::create_dir_all(p.parent().expect("parent")).expect("dir");
        fs::write(p, b"x").expect("file");
    }
}

/// The records whose contents the resume check reads, as a finished step leaves them.
fn write_records(step: StepName, work: &WorkDir) {
    match step {
        StepName::Output => fs::write(
            work.output_record(),
            br#"{"path":"a.srt","unchanged":false}"#,
        )
        .unwrap(),
        StepName::LocalizedVideo => fs::write(
            work.text(step),
            br#"{"path":null,"encoder":"","frames":0,"replaced":0}"#,
        )
        .unwrap(),
        _ => {}
    }
}

#[test]
fn missing_visual_crop_invalidates_visual_descendants_without_repeating_audio() {
    let work = scratch("missing-visual-crop");
    let mut r = record();
    r.video = work
        .root()
        .join("episode.mp4")
        .to_string_lossy()
        .into_owned();
    for (index, step) in StepName::ALL.into_iter().enumerate() {
        touch(&graph::outputs(
            step,
            &work,
            Path::new(&r.video),
            r.settings.effective_output_format(),
        ));
        if step == StepName::TextDetect {
            fs::write(
                work.text(step),
                br#"{"occurrences":[{"crops":["visual/crops/one.png"]}]}"#,
            )
            .unwrap();
            touch(&[work.root().join("visual/crops/one.png")]);
        }
        write_records(step, &work);
        r.steps.insert(
            step,
            StepRecord {
                fingerprint: fingerprint_in_work(step, &r, &work),
                finished_ns: index as u128 + 1,
                measure: StepMeasure::default(),
            },
        );
    }
    assert!(stale_steps(&r, &work).is_empty());
    fs::remove_file(work.root().join("visual/crops/one.png")).unwrap();
    assert_eq!(
        stale_steps(&r, &work),
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
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_finished_step_with_its_files_is_reused_and_a_missing_file_reruns_it() {
    let work = scratch("files");
    let mut r = record();
    finish(&mut r, StepName::ProbeDecode, 1);
    assert!(!is_valid(StepName::ProbeDecode, &r, &work), "no files yet");
    touch(&[work.probe(), work.mix()]);
    assert!(is_valid(StepName::ProbeDecode, &r, &work));
    fs::remove_file(work.mix()).expect("remove");
    assert!(!is_valid(StepName::ProbeDecode, &r, &work));
    // A part file left by a killed run is not an output.
    touch(&[work.mix().with_extension("f32.part")]);
    assert!(!is_valid(StepName::ProbeDecode, &r, &work));
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_rerun_upstream_step_invalidates_every_step_that_reads_it() {
    let mut r = record();
    finish(&mut r, StepName::ProbeDecode, 1);
    finish(&mut r, StepName::Separation, 2);
    finish(&mut r, StepName::Vad, 3);
    let vad = fingerprint(StepName::Vad, &r);
    assert_eq!(vad, r.steps[&StepName::Vad].fingerprint);
    // Separation runs again: same settings, a new finish time.
    finish(&mut r, StepName::Separation, 9);
    assert_ne!(fingerprint(StepName::Vad, &r), vad);
    // The shot scan reads nothing but the video and is untouched.
    let shots = fingerprint(StepName::ShotScan, &r);
    finish(&mut r, StepName::ProbeDecode, 10);
    assert_eq!(fingerprint(StepName::ShotScan, &r), shots);
}

#[test]
fn a_setting_changes_only_the_steps_that_read_it_and_the_video_changes_the_first() {
    let r = record();
    let mut tuned = r.clone();
    tuned.settings.cut_score = 35.0;
    assert_ne!(
        fingerprint(StepName::Cues, &r),
        fingerprint(StepName::Cues, &tuned)
    );
    assert_eq!(
        fingerprint(StepName::Alignment, &r),
        fingerprint(StepName::Alignment, &tuned)
    );
    let mut touched = r.clone();
    touched.video_modified_s = 6;
    assert_ne!(
        fingerprint(StepName::ProbeDecode, &r),
        fingerprint(StepName::ProbeDecode, &touched)
    );
    assert_eq!(
        fingerprint(StepName::Vad, &r),
        fingerprint(StepName::Vad, &touched)
    );
}

#[test]
fn a_live_lock_refuses_a_second_run_and_a_dead_one_is_taken_over() {
    let work = scratch("lock");
    // Our own parent is alive and is not us.
    let parent = std::os::unix::process::parent_id();
    fs::write(work.lock(), parent.to_string()).expect("lock");
    assert!(lock(&work).is_err());
    fs::write(work.lock(), "999999999").expect("stale");
    let held = lock(&work).expect("take over");
    assert_eq!(
        fs::read_to_string(work.lock()).expect("read"),
        std::process::id().to_string()
    );
    drop(held);
    assert!(!work.lock().exists());
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn the_stale_steps_are_the_invalid_ones_and_everything_that_reads_them() {
    let work = scratch("stale");
    let mut r = record();
    r.video = work.root().join("a.mp4").to_string_lossy().into_owned();
    assert_eq!(stale_steps(&r, &work), StepName::ALL.to_vec(), "a new job");
    for (ns, step) in StepName::ALL.into_iter().enumerate() {
        finish(&mut r, step, ns as u128 + 1);
        touch(&graph::outputs(
            step,
            &work,
            Path::new(&r.video),
            r.settings.effective_output_format(),
        ));
        if step == StepName::TextDetect {
            fs::write(work.text(step), br#"{"occurrences":[]}"#).unwrap();
        }
        write_records(step, &work);
    }
    assert!(stale_steps(&r, &work).is_empty(), "a finished job");
    r.steps.remove(&StepName::Cues);
    assert_eq!(
        stale_steps(&r, &work),
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
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn the_corrections_reach_the_review_step_alone() {
    let mut a = record();
    let before: Vec<String> = StepName::ALL.iter().map(|s| fingerprint(*s, &a)).collect();
    a.corrections = Some("abc".into());
    for (step, old) in StepName::ALL.iter().zip(&before) {
        let changed = fingerprint(*step, &a) != *old;
        assert_eq!(changed, *step == StepName::Review, "{step}");
    }
}
