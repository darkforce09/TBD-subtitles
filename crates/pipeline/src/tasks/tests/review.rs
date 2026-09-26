use job_model::job::{JobRecord, JobSettings};
use job_model::outputs::{AlignedWord, Chosen, Correction, TimingSource};

use super::*;
use crate::work_dir::WorkDir;

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
    let dir = std::env::temp_dir().join(format!("tbd-review-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let work = WorkDir::new(&dir);
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
    work_dir::write_json(&work.aligned(), &aligned).expect("aligned");
    let job = Job {
        work: work.clone(),
        record: JobRecord {
            video: "a.mp4".into(),
            video_size: 1,
            video_modified_s: 0,
            settings: JobSettings::with_glossary(vec![]),
            models_dir: Some("/no/models".into()),
            corrections: None,
            steps: Default::default(),
        },
    };
    let report = review(&job).expect("review");
    assert_eq!(
        report.notes.get("corrections").map(String::as_str),
        Some("0")
    );
    let reviewed: Aligned = work_dir::read_json(&work.reviewed()).expect("reviewed");
    assert_eq!(reviewed, aligned);
    let _ = std::fs::remove_dir_all(&dir);
}
