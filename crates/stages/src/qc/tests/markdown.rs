use std::collections::BTreeMap;

use job_model::job::{JobSettings, StepMeasure, StepRecord};
use job_model::report::{QcCheck, QcFinding, QcSummary};

use super::*;

fn record() -> JobRecord {
    let mut steps = BTreeMap::new();
    steps.insert(
        StepName::Separation,
        StepRecord {
            fingerprint: "f".into(),
            finished_ns: 1,
            measure: StepMeasure {
                wall_s: 100.0,
                load_s: Some(0.9),
                process_s: Some(99.0),
                peak_ram_mib: Some(1156.0),
                peak_vram_mib: Some(4280.0),
                ..StepMeasure::default()
            },
        },
    );
    steps.insert(
        StepName::Vad,
        StepRecord {
            fingerprint: "g".into(),
            finished_ns: 2,
            measure: StepMeasure {
                wall_s: 0.6,
                ..StepMeasure::default()
            },
        },
    );
    JobRecord {
        video: "/v/[Muhn Pace] Dressrosa 11.mp4".into(),
        video_size: 1,
        video_modified_s: 0,
        settings: JobSettings::with_glossary(vec![]),
        steps,
    }
}

#[test]
fn the_report_lists_flags_steps_and_unmeasured_values_as_dashes() {
    let report = QcReport {
        summary: QcSummary {
            cues: 3,
            cps_ok_share: 0.9,
            video_s: 1853.7,
            offset_ms: Some(-12.0),
            ..QcSummary::default()
        },
        findings: vec![QcFinding {
            check: QcCheck::Unsure,
            time_s: 725.44,
            text: "Law | the Birdcage".into(),
            detail: "U0412".into(),
        }],
    };
    let md = render(
        &report,
        &record(),
        "/v/[Muhn Pace] Dressrosa 11.srt",
        &["12.0s [thud]".into()],
    );
    assert!(md.starts_with("# Job report: [Muhn Pace] Dressrosa 11.mp4\n"));
    assert!(md.contains("misses the 95 % target"));
    assert!(
        md.contains("| 0:12:05.4 | unsure after re-decode | Law \\| the Birdcage | U0412 |"),
        "{md}"
    );
    assert!(
        md.contains("| separation | separation | 100.0 | 0.9 | 99.0 | 1156 | — | 4280 |"),
        "{md}"
    );
    assert!(
        md.contains("| vad | vad | 0.6 | — | — | — | — | — |"),
        "{md}"
    );
    assert!(md.contains("- 12.0s [thud]"));
    assert!(md.contains("-12 ms"));
}

#[test]
fn clock_reads_hours_minutes_seconds_and_tenths() {
    assert_eq!(clock(0.0), "0:00:00.0");
    assert_eq!(clock(3725.46), "1:02:05.5");
}
