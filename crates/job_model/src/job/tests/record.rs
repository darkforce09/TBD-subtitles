use super::*;
use crate::job::JobSettings;

#[test]
fn a_record_round_trips_through_json() {
    let record = JobRecord {
        video: "/videos/a.mp4".to_string(),
        video_size: 10,
        video_modified_s: 1_700_000_000,
        settings: JobSettings::with_glossary(vec!["Luffy".to_string()]),
        models_dir: None,
        corrections: None,
    };
    let json = serde_json::to_string(&record).expect("json");
    assert!(json.contains("\"separator\":\"roformer\""));
    let back: JobRecord = serde_json::from_str(&json).expect("parse");
    assert_eq!(back, record);
}

#[test]
fn a_record_without_its_optional_fields_parses() {
    let json = r#"{"video":"v","video_size":1,"video_modified_s":0,"settings":{"separator":"mdx_net","whisper":"large_v3_turbo","audio_track":1,"glossary":[],"llm_model":"sonnet","llm_processes":2,"cut_score":25.0}}"#;
    let record: JobRecord = serde_json::from_str(json).expect("parse");
    assert_eq!(record.settings.audio_track, Some(1));
    assert_eq!(record.settings.output_format, crate::job::OutputFormat::Srt);
    assert_eq!(record.models_dir, None);
}

#[test]
fn step_records_round_trip_through_json_by_step_name() {
    let steps = StepRecords::from([(
        StepName::AsrWhisper,
        StepRecord {
            fingerprint: "ab".to_string(),
            finished_ns: 5,
            measure: StepMeasure {
                wall_s: 1.5,
                peak_vram_mib: Some(4412.0),
                ..StepMeasure::default()
            },
        },
    )]);
    let json = serde_json::to_string(&steps).expect("json");
    assert!(json.contains("\"asr_whisper\":"));
    let back: StepRecords = serde_json::from_str(&json).expect("parse");
    assert_eq!(back, steps);
}
