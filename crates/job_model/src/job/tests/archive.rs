use std::collections::BTreeMap;
use std::ops::ControlFlow;
use std::path::PathBuf;

use rkyv::rancor::Error;

use super::{JobRecord, JobRun, JobSettings, OutputFormat, Separator, StepMeasure, StepRecord};
use super::{StepRecords, WhisperModel, WorkerMeasure};
use crate::archive_round_trip::{misaligned, round_trip};
use crate::onscreen::TextSettings;
use crate::stage::{ArchivedStepName, StepName};

fn notes() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("chunks".to_string(), "42".to_string()),
        ("calls".to_string(), "7 of 8 ドレスローザ".to_string()),
    ])
}

fn measure() -> StepMeasure {
    StepMeasure {
        wall_s: 12.5,
        load_s: Some(1.25),
        process_s: Some(11.0),
        peak_ram_mib: Some(2048.5),
        peak_child_ram_mib: Some(512.0),
        peak_vram_mib: Some(4096.0),
        gpu_busy_pct: Some(87.5),
        gpu_encoder_pct: Some(41.0),
        gpu_decoder_pct: None,
        job_ram_mib: Some(6144.0),
        cpu_cores_mean: Some(3.25),
        cpu_cores_peak: Some(7.5),
        busiest_thread_pct: Some(99.0),
        notes: notes(),
    }
}

fn step(n: usize) -> StepRecord {
    StepRecord {
        fingerprint: format!("sha256-{n:04}"),
        finished_ns: 1_790_000_000_000_000_000_000 + n as u128,
        measure: measure(),
    }
}

fn settings() -> JobSettings {
    JobSettings {
        separator: Separator::MdxNet,
        whisper: WhisperModel::LargeV3Turbo,
        audio_track: Some(2),
        glossary: vec!["Doflamingo".into(), "Rebecca".into()],
        llm_model: "sonnet".into(),
        llm_processes: 8,
        cut_score: 20.0,
        output_format: OutputFormat::Ass,
        onscreen_text: TextSettings {
            enabled: true,
            claude_fallback: false,
            reference_folder: Some(PathBuf::from("/media/one pace/ドレスローザ/reference")),
            localized_video: true,
            ..TextSettings::default()
        },
    }
}

fn record() -> JobRecord {
    JobRecord {
        video: "/media/one pace/Dressrosa 11.mkv".into(),
        video_size: 677_000_000,
        video_modified_s: 1_790_000_000,
        settings: settings(),
        models_dir: Some("/models".into()),
        corrections: Some("abc123".into()),
    }
}

fn steps() -> StepRecords {
    StepName::ALL
        .into_iter()
        .enumerate()
        .map(|(n, name)| (name, step(n)))
        .collect()
}

#[test]
fn job_record_round_trips() {
    round_trip(&record());
}

#[test]
fn step_records_round_trip() {
    round_trip(&steps());
}

#[test]
fn step_record_round_trips() {
    round_trip(&step(3));
}

#[test]
fn step_measure_round_trips() {
    round_trip(&measure());
}

#[test]
fn job_run_round_trips() {
    let run = JobRun {
        started_ns: 1_790_000_000_000_000_000_000,
        finished_ns: 1_790_000_001_500_000_000_000,
        peak_ram_mib: Some(9216.5),
    };
    round_trip(&run);
    round_trip(&JobRun::default());
}

#[test]
fn a_job_run_s_wall_time_is_its_span_in_seconds() {
    let run = JobRun {
        started_ns: 2_000_000_000,
        finished_ns: 3_500_000_000,
        peak_ram_mib: None,
    };
    assert_eq!(run.wall_s(), 1.5);
}

#[test]
fn worker_measure_round_trips() {
    round_trip(&WorkerMeasure {
        load_s: 1.5,
        process_s: 30.25,
        peak_ram_mib: 900.0,
        peak_child_ram_mib: 120.0,
        notes: notes(),
    });
}

/// Every separator; [`listed_separator`] keeps the list complete.
const EVERY_SEPARATOR: [Separator; 2] = [Separator::Roformer, Separator::MdxNet];

/// Every Whisper model; [`listed_whisper`] keeps the list complete.
const EVERY_WHISPER: [WhisperModel; 2] = [WhisperModel::LargeV3, WhisperModel::LargeV3Turbo];

/// Fails to compile when `Separator` gains a variant, until [`EVERY_SEPARATOR`] lists it too.
fn listed_separator(separator: Separator) {
    match separator {
        Separator::Roformer | Separator::MdxNet => {}
    }
}

/// Fails to compile when `WhisperModel` gains a variant, until [`EVERY_WHISPER`] lists it too.
fn listed_whisper(whisper: WhisperModel) {
    match whisper {
        WhisperModel::LargeV3 | WhisperModel::LargeV3Turbo => {}
    }
}

#[test]
fn job_settings_round_trip_with_every_choice() {
    for separator in EVERY_SEPARATOR {
        listed_separator(separator);
        for whisper in EVERY_WHISPER {
            listed_whisper(whisper);
            for output_format in OutputFormat::ALL {
                round_trip(&JobSettings {
                    separator,
                    whisper,
                    output_format,
                    ..settings()
                });
            }
        }
    }
}

#[test]
fn settings_enums_round_trip() {
    round_trip(&Separator::Roformer);
    round_trip(&Separator::MdxNet);
    round_trip(&WhisperModel::LargeV3);
    round_trip(&WhisperModel::LargeV3Turbo);
    for format in OutputFormat::ALL {
        round_trip(&format);
    }
}

#[test]
fn archived_steps_are_found_by_archived_step_name() {
    let job = steps();
    let bytes = rkyv::to_bytes::<Error>(&job).unwrap();
    let (buffer, start) = misaligned(&bytes);
    let archived =
        rkyv::access::<rkyv::Archived<StepRecords>, Error>(&buffer[start..start + bytes.len()])
            .unwrap();

    let found = archived
        .get(&ArchivedStepName::Readjudicate)
        .expect("the step is in the archived map");
    assert_eq!(
        found.fingerprint.as_str(),
        job[&StepName::Readjudicate].fingerprint
    );
    assert_eq!(
        found.finished_ns.to_native(),
        job[&StepName::Readjudicate].finished_ns
    );

    let mut expected = StepName::ALL.into_iter();
    let mut visited = 0;
    archived.visit(|name, _| {
        assert_eq!(name, &expected.next().expect("no more keys than steps"));
        visited += 1;
        ControlFlow::<()>::Continue(())
    });
    assert_eq!(visited, StepName::ALL.len());
}
