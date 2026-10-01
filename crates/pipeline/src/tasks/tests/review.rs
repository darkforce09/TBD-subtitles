use job_model::job::{JobRecord, JobSettings, StepMeasure, StepRecord};
use job_model::outputs::{AlignedWord, Chosen, Correction, TimingSource};

use super::*;
use crate::work_dir::store::scratch::Scratch;

/// A record for a step a test ran.
fn finished() -> StepRecord {
    StepRecord {
        fingerprint: "test".into(),
        finished_ns: 1,
        measure: StepMeasure::default(),
    }
}

fn line(id: &str, t: &str, f: &[&str]) -> Line {
    Line {
        id: id.into(),
        t: t.into(),
        f: f.iter().map(|s| s.to_string()).collect(),
    }
}

fn correction(id: &str, text: &str, flags: &[&str]) -> Correction {
    Correction {
        id: id.into(),
        text: text.into(),
        flags: flags.iter().map(|s| s.to_string()).collect(),
        chosen: Chosen::Typed,
    }
}

#[test]
fn a_correction_replaces_the_text_and_flags_of_its_line_only() {
    let lines = vec![
        line("U1", "Blaver!", &["UNSURE"]),
        line("U2", "Go!", &["SPK"]),
    ];
    let corrections = Corrections {
        lines: vec![correction("U1", "Bravo!", &["SPK"])],
    };
    let out = corrected_lines(&lines, &corrections);
    assert_eq!(out[0], line("U1", "Bravo!", &["SPK"]));
    assert_eq!(out[1], lines[1]);
}

#[test]
fn with_no_corrections_the_reviewed_words_are_the_aligned_words() {
    let scratch = Scratch::new("review-none");
    let aligned = Aligned {
        utterances: vec![AlignedUtterance {
            id: "U1".into(),
            words: vec![AlignedWord {
                text: "Go!".into(),
                start_s: 1.0,
                end_s: 1.4,
                source: TimingSource::Ctc,
            }],
            speaker_starts: vec![],
            new_speaker: false,
            narrator: false,
            unsure: false,
        }],
        blocks: 1,
        failed_blocks: 0,
        offset_s: None,
        errors: vec![],
    };
    scratch
        .store()
        .put_output(StepName::Alignment, None, &aligned)
        .expect("aligned");
    let job = Job {
        work: scratch.work().clone(),
        record: JobRecord {
            video: "a.mp4".into(),
            video_size: 1,
            video_modified_s: 0,
            settings: JobSettings::with_glossary(vec![]),
            models_dir: Some("/no/models".into()),
            corrections: None,
        },
    };
    let mut io = StepIo::in_process(scratch.store()).expect("io");
    let report = review(&job, &mut io, &|_, _| {}).expect("review");
    io.into_outputs()
        .expect("the pending write")
        .commit(StepName::Review, &finished())
        .expect("commit");
    assert_eq!(
        report.notes.get("corrections").map(String::as_str),
        Some("0")
    );
    let reviewed: Option<Aligned> = scratch
        .store()
        .read()
        .expect("read")
        .output(StepName::Review, None)
        .expect("reviewed");
    assert_eq!(reviewed, Some(aligned));
}
