use job_model::job::{OutputFormat, Separator, WhisperModel};

use super::*;
use crate::settings::models::app_settings::Backend;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-settings-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

#[test]
fn an_empty_file_is_the_measured_defaults() {
    let settings = parse("").expect("parse");
    assert_eq!(settings, AppSettings::default());
    assert_eq!(settings.glossary, "one_piece");
    assert_eq!(settings.engines.separator, Separator::Roformer);
    assert_eq!(settings.engines.whisper, WhisperModel::LargeV3);
    assert_eq!(settings.language_model.backend, Backend::Claude);
    assert_eq!(settings.language_model.model, "sonnet");
    assert_eq!(settings.language_model.processes, 8);
    assert_eq!(settings.cut_score, 20.0);
    assert_eq!(settings.output_format, OutputFormat::Srt);
    assert_eq!((settings.models_dir, settings.work_root), (None, None));
}

#[test]
fn every_key_is_read() {
    let settings = parse(
        r#"
models_dir = "/models"
work_root = "/big/work"
glossary = "none"
cut_score = 25.5
output_format = "ass"

[engines]
separator = "mdx_net"
whisper = "large_v3_turbo"

[language_model]
backend = "claude"
model = "opus"
processes = 4
"#,
    )
    .expect("parse");
    assert_eq!(settings.models_dir, Some(PathBuf::from("/models")));
    assert_eq!(settings.work_root, Some(PathBuf::from("/big/work")));
    assert_eq!(settings.glossary, "none");
    assert_eq!(settings.cut_score, 25.5);
    assert_eq!(settings.output_format, OutputFormat::Ass);
    assert_eq!(settings.engines.separator, Separator::MdxNet);
    assert_eq!(settings.engines.whisper, WhisperModel::LargeV3Turbo);
    assert_eq!(settings.language_model.model, "opus");
    assert_eq!(settings.language_model.processes, 4);
}

#[test]
fn a_missing_table_key_keeps_the_others_default() {
    let settings = parse("[engines]\nwhisper = \"large_v3_turbo\"\n").expect("parse");
    assert_eq!(settings.engines.whisper, WhisperModel::LargeV3Turbo);
    assert_eq!(settings.engines.separator, Separator::Roformer);
}

#[test]
fn an_unknown_key_is_an_error_naming_it() {
    let error = parse("cut_scor = 30\n").expect_err("unknown");
    assert!(error.contains("cut_scor"), "{error}");
    let error = parse("[engines]\nseparater = \"roformer\"\n").expect_err("unknown");
    assert!(error.contains("separater"), "{error}");
}

#[test]
fn a_bad_value_is_an_error_naming_it() {
    let error = parse("output_format = \"txt\"\n").expect_err("bad");
    assert!(error.contains("txt"), "{error}");
    assert!(parse("[language_model]\nbackend = \"mistral\"\n").is_err());
    assert!(parse("cut_score = \"high\"\n").is_err());
}

#[test]
fn saved_settings_load_back_unchanged() {
    let dir = scratch("round-trip");
    let path = dir.join("tbd-subtitles").join("settings.toml");
    let mut settings = AppSettings {
        models_dir: Some(PathBuf::from("/models")),
        output_format: OutputFormat::Vtt,
        ..AppSettings::default()
    };
    settings.engines.separator = Separator::MdxNet;
    save(&path, &settings).expect("save");
    assert!(!path.with_extension("toml.part").exists());
    assert_eq!(load(&path).expect("load"), settings);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_missing_file_is_the_defaults_and_a_broken_one_an_error() {
    let dir = scratch("missing");
    let path = dir.join("settings.toml");
    assert_eq!(load(&path).expect("load"), AppSettings::default());
    fs::create_dir_all(&dir).expect("dir");
    fs::write(&path, "cut_score = [").expect("write");
    let error = load(&path).expect_err("broken").to_string();
    assert!(error.contains("settings.toml"), "{error}");
    let _ = fs::remove_dir_all(&dir);
}
