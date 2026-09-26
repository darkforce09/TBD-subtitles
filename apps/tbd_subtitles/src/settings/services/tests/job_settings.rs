use job_model::job::{OutputFormat, Separator, WhisperModel};

use super::*;

#[test]
fn the_defaults_ask_for_the_measured_stack_and_the_built_in_glossary() {
    let job = job_settings(&AppSettings::default()).expect("settings");
    assert!(job.glossary.iter().any(|t| t == "Doflamingo"));
    assert_eq!(job.separator, Separator::Roformer);
    assert_eq!(job.whisper, WhisperModel::LargeV3);
    assert_eq!(job.cut_score, 20.0);
    assert_eq!(job.llm_model, "sonnet");
    assert_eq!(job.llm_processes, 8);
    assert_eq!(job.output_format, OutputFormat::Srt);
}

#[test]
fn every_setting_reaches_the_job() {
    let mut settings = AppSettings {
        glossary: "none".into(),
        cut_score: 30.0,
        output_format: OutputFormat::Vtt,
        ..AppSettings::default()
    };
    settings.engines.separator = Separator::MdxNet;
    settings.engines.whisper = WhisperModel::LargeV3Turbo;
    settings.language_model.model = "opus".into();
    settings.language_model.processes = 0;
    let job = job_settings(&settings).expect("settings");
    assert!(job.glossary.is_empty());
    assert_eq!(job.separator, Separator::MdxNet);
    assert_eq!(job.whisper, WhisperModel::LargeV3Turbo);
    assert_eq!(job.cut_score, 30.0);
    assert_eq!(job.llm_model, "opus");
    assert_eq!(job.llm_processes, 1, "at least one process runs");
    assert_eq!(job.output_format, OutputFormat::Vtt);
}

#[test]
fn a_missing_glossary_file_is_an_error_naming_it() {
    let settings = AppSettings {
        glossary: "/no/such.json".into(),
        ..AppSettings::default()
    };
    let error = job_settings(&settings).expect_err("missing");
    assert!(format!("{error:#}").contains("/no/such.json"));
}

#[test]
fn named_folders_win_over_the_defaults() {
    let settings = AppSettings {
        models_dir: Some(PathBuf::from("/models")),
        work_root: Some(PathBuf::from("/work")),
        ..AppSettings::default()
    };
    assert_eq!(
        models_dir(&settings).expect("models"),
        PathBuf::from("/models")
    );
    assert_eq!(work_root(&settings).expect("work"), PathBuf::from("/work"));
}
