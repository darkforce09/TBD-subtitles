use std::collections::BTreeMap;

use job_model::job::{JobSettings, StepMeasure, StepRecord};
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

fn record_with(notes: Option<&[(&str, &str)]>) -> JobRecord {
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
    JobRecord {
        video: "/videos/a.mkv".into(),
        video_size: 10,
        video_modified_s: 5,
        settings: JobSettings::with_glossary(Vec::new()),
        models_dir: None,
        corrections: None,
        steps,
    }
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
    assert!(!localized_video_ran(&record_with(None)));
    assert!(!localized_video_ran(&record_with(Some(&[(
        "disabled", "true"
    )]))));
    assert!(localized_video_ran(&record_with(Some(&[
        ("encoder", "libx264"),
        ("frames", "100"),
    ]))));
}
