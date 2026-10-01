use std::collections::BTreeMap;

use job_model::job::{StepMeasure, StepRecord};
use job_model::onscreen::ReplacedText;

use super::*;

fn replaced(id: &str, status: ReplaceStatus) -> ReplacedText {
    ReplacedText {
        id: id.into(),
        first_frame: 0,
        last_frame: 10,
        status,
        style: None,
        container: None,
        plates: Vec::new(),
        preview: None,
        lettering_quad: None,
    }
}

fn steps_with(notes: Option<&[(&str, &str)]>) -> StepRecords {
    let mut steps = BTreeMap::new();
    if let Some(notes) = notes {
        steps.insert(
            StepName::LocalizedVideo,
            StepRecord {
                fingerprint: "f".into(),
                finished_ns: 1,
                measure: StepMeasure {
                    notes: notes
                        .iter()
                        .map(|(key, value)| (key.to_string(), value.to_string()))
                        .collect(),
                    ..StepMeasure::default()
                },
            },
        );
    }
    steps
}

#[test]
fn the_localized_video_lines_count_replacements_and_fallbacks_and_name_the_file() {
    let composed = ReplacementDocument {
        width: 1920,
        height: 1080,
        frame_count: 100,
        texts: vec![
            replaced("T1", ReplaceStatus::Baked),
            replaced("T2", ReplaceStatus::Fallback("busy background".into())),
            replaced("T3", ReplaceStatus::Baked),
            replaced("T4", ReplaceStatus::Fallback("curved writing".into())),
            replaced("T5", ReplaceStatus::Fallback("too small".into())),
        ],
    };
    let video = LocalizedVideoRecord {
        path: Some("/videos/a.localized.mkv".into()),
        encoder: "hevc_nvenc".into(),
        frames: 100,
        replaced: 2,
        earlier: None,
    };
    assert_eq!(
        localized_lines(&composed, &video),
        "\n### Localized video\n\n\
         - Occurrences replaced in the video: 2\n\
         - Occurrences left in Japanese: 3\n\
         - Localized video: /videos/a.localized.mkv\n\
         - Encoder: hevc_nvenc\n"
    );
}

#[test]
fn the_section_appears_only_when_the_step_ran_enabled() {
    assert!(!localized_video_ran(&steps_with(None)));
    assert!(!localized_video_ran(&steps_with(Some(&[(
        "disabled", "true"
    )]))));
    assert!(localized_video_ran(&steps_with(Some(&[
        ("encoder", "libx264"),
        ("frames", "100"),
    ]))));
}

#[test]
fn the_report_renders_the_stored_check_and_dropped_sounds_and_names_a_missing_one() {
    use crate::work_dir::store::scratch::{Scratch, job_record};
    let scratch = Scratch::new("report-write");
    let (store, mut record) = (scratch.store(), job_record(&scratch.dir));
    record.settings.onscreen_text.enabled = false;
    let qc = QcReport::default();
    store.put_output(StepName::Qc, None, &qc).unwrap();
    let error = write(scratch.work(), &record, &steps_with(None)).expect_err("no dropped sounds");
    assert!(error.to_string().contains("cues/dropped_sounds"), "{error}");
    let dropped = vec!["[THUNDER]".to_string()];
    store
        .put_output(StepName::Cues, Some(keys::DROPPED_SOUNDS), &dropped)
        .unwrap();
    assert_eq!(
        write(scratch.work(), &record, &steps_with(None)).unwrap(),
        qc
    );
    let report = std::fs::read_to_string(scratch.work().report()).unwrap();
    assert!(report.contains("[THUNDER]"), "{report}");
}
