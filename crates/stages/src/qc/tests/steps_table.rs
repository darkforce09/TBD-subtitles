use std::collections::BTreeMap;

use job_model::job::StepRecord;

use super::*;

fn record(measure: StepMeasure) -> StepRecord {
    StepRecord {
        fingerprint: "f".into(),
        finished_ns: 1,
        measure,
    }
}

fn notes(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn measured_steps() -> StepRecords {
    StepRecords::from([
        (
            StepName::ProbeDecode,
            record(StepMeasure {
                wall_s: 2.0,
                notes: notes(&[("fps", "23.976")]),
                ..StepMeasure::default()
            }),
        ),
        (
            StepName::ShotScan,
            record(StepMeasure {
                wall_s: 100.0,
                ..StepMeasure::default()
            }),
        ),
        (
            StepName::TextDetect,
            record(StepMeasure {
                wall_s: 240.0,
                load_s: Some(3.0),
                process_s: Some(200.0),
                cpu_cores_mean: Some(2.5),
                cpu_cores_peak: Some(6.5),
                busiest_thread_pct: Some(97.0),
                gpu_busy_pct: Some(64.0),
                gpu_decoder_pct: Some(0.0),
                job_ram_mib: Some(3500.0),
                notes: notes(&[
                    ("decode_wait_s", "50.0"),
                    ("detect_s", "100.0"),
                    ("confirm_s", "10.0"),
                    ("stills_s", "20.0"),
                    ("frames_decoded", "40000"),
                    ("frames_screened", "4000"),
                ]),
                ..StepMeasure::default()
            }),
        ),
        (
            StepName::TextVerify,
            record(StepMeasure {
                wall_s: 30.0,
                process_s: Some(25.0),
                notes: notes(&[("samples", "50")]),
                ..StepMeasure::default()
            }),
        ),
        (
            StepName::LocalizedVideo,
            record(StepMeasure {
                wall_s: 300.0,
                process_s: Some(300.0),
                gpu_encoder_pct: Some(55.0),
                job_ram_mib: Some(5200.0),
                notes: notes(&[
                    ("decode_wait_s", "30.0"),
                    ("blend_s", "60.0"),
                    ("encode_wait_s", "180.0"),
                    ("flush_s", "3.0"),
                    ("frames", "45000"),
                ]),
                ..StepMeasure::default()
            }),
        ),
    ])
}

#[test]
fn a_row_shows_speed_cpu_gpu_and_the_job_s_memory() {
    let mut md = String::new();
    steps_section(&mut md, &measured_steps(), 1800.0, None);
    assert!(
        md.contains(
            "| text_detect | onscreen_text | 240.0 | 7.5 | 3.0 | 200.0 | 2.5 / 6.5 | 97 | 64 | — | — | 3500 | — |"
        ),
        "{md}"
    );
    assert!(
        md.contains(
            "| shot_scan | probe_decode | 100.0 | 18.0 | — | — | — | — | — | — | — | — | — |"
        ),
        "{md}"
    );
}

#[test]
fn the_footer_gives_the_real_wall_time_and_the_whole_job_peak() {
    let run = JobRun {
        started_ns: 0,
        finished_ns: 600_000_000_000,
        peak_ram_mib: Some(6000.0),
    };
    let mut md = String::new();
    steps_section(&mut md, &measured_steps(), 1800.0, Some(&run));
    assert!(
        md.contains("Real wall time of the last run 10.0 min against 11.2 min of summed step time"),
        "{md}"
    );
    assert!(
        md.contains("5200 MiB across the steps, 6000 MiB over the last run."),
        "{md}"
    );
}

#[test]
fn without_a_run_the_footer_shows_dashes() {
    let mut md = String::new();
    steps_section(&mut md, &StepRecords::new(), 0.0, None);
    assert!(md.contains("Real wall time of the last run — min"), "{md}");
    assert!(
        md.contains("— MiB across the steps, — MiB over the last run."),
        "{md}"
    );
}

#[test]
fn the_phase_section_splits_detection_and_the_localized_video() {
    let mut md = String::new();
    phase_section(&mut md, &measured_steps(), 1800.0);
    assert!(md.starts_with("\n## Phase times\n"), "{md}");
    assert!(
        md.contains(
            "- text_detect: decode wait 50.0 s (25 %); detection 100.0 s (50 %); confirmation 10.0 s (5 %); stills 20.0 s (10 %); 40000 frames decoded (200 fps); 4000 frames screened (20 fps)\n"
        ),
        "{md}"
    );
    assert!(
        md.contains(
            "- localized_video: decode wait 30.0 s (10 %); blend 60.0 s (20 %); encode wait 180.0 s (60 %); flush 3.0 s (1 %); 45000 frames (150.0 fps); NVENC 55 % mean\n"
        ),
        "{md}"
    );
    assert!(
        md.contains("- shot_scan: 43157 frames at 432 fps\n"),
        "{md}"
    );
    assert!(
        md.contains("- text_verify: 50 samples at 2.0 per second\n"),
        "{md}"
    );
}

#[test]
fn nvdec_shows_only_when_it_was_used() {
    let mut steps = measured_steps();
    let mut md = String::new();
    phase_section(&mut md, &steps, 1800.0);
    assert!(!md.contains("NVDEC"), "{md}");
    if let Some(detect) = steps.get_mut(&StepName::TextDetect) {
        detect.measure.gpu_decoder_pct = Some(12.0);
    }
    let mut md = String::new();
    phase_section(&mut md, &steps, 1800.0);
    assert!(md.contains("; NVDEC 12 % mean\n"), "{md}");
}

#[test]
fn no_phase_section_without_phases() {
    let mut md = String::new();
    phase_section(&mut md, &StepRecords::new(), 1800.0);
    assert!(md.is_empty());
}
