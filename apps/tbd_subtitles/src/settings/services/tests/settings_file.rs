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
    assert_eq!(settings.language_model.fix_model, "opus");
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
fix_model = "fable"
processes = 4
fix_calls = 64
fix_after_run = true
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
    assert_eq!(settings.language_model.fix_model, "fable");
    assert_eq!(settings.language_model.processes, 4);
    assert_eq!(settings.language_model.fix_calls, 64);
    assert!(settings.language_model.fix_after_run);
}

#[test]
fn a_file_from_before_fix_it_keeps_its_model_and_asks_opus_to_fix() {
    let settings =
        parse("[language_model]\nbackend = \"claude\"\nmodel = \"haiku\"\nprocesses = 2\n")
            .expect("parse");
    assert_eq!(settings.language_model.model, "haiku");
    assert_eq!(settings.language_model.fix_model, "opus");
}

#[test]
fn a_file_from_before_fix_it_ran_at_scale_makes_32_calls_and_waits_to_be_asked() {
    let settings = parse(
        "[language_model]\nbackend = \"claude\"\nmodel = \"sonnet\"\nfix_model = \"opus\"\n\
         processes = 8\n",
    )
    .expect("parse");
    assert_eq!(settings.language_model.fix_calls, 32);
    assert!(!settings.language_model.fix_after_run);
}

#[test]
fn a_missing_table_key_keeps_the_others_default() {
    let settings = parse("[engines]\nwhisper = \"large_v3_turbo\"\n").expect("parse");
    assert_eq!(settings.engines.whisper, WhisperModel::LargeV3Turbo);
    assert_eq!(settings.engines.separator, Separator::Roformer);
}

#[test]
fn a_file_from_before_the_localized_video_replaces_text_in_the_video() {
    let settings = parse(
        "[onscreen_text]\nenabled = true\nclaude_fallback = false\n\
         reference_folder = \"/refs\"\n",
    )
    .expect("parse");
    let text = &settings.onscreen_text;
    assert!(text.localized_video, "a missing key means on");
    assert!(text.enabled && !text.claude_fallback);
    assert_eq!(text.reference_folder, Some(PathBuf::from("/refs")));
    assert!(parse("").expect("parse").onscreen_text.localized_video);
    let off = parse("[onscreen_text]\nlocalized_video = false\n").expect("parse");
    assert!(
        !off.onscreen_text.localized_video,
        "a saved false stays off"
    );
    let on = parse("[onscreen_text]\nlocalized_video = true\n").expect("parse");
    assert!(on.onscreen_text.localized_video);
    assert!(parse("[onscreen_text]\nlocalized_video = \"yes\"\n").is_err());
}

#[test]
fn a_saved_choice_to_leave_the_video_alone_loads_back_off() {
    let dir = scratch("localized-off");
    let path = dir.join("tbd-subtitles").join("settings.toml");
    let mut settings = AppSettings::default();
    settings.onscreen_text.localized_video = false;
    save(&path, &settings).expect("save");
    assert_eq!(load(&path).expect("load"), settings);
    settings.onscreen_text.localized_video = true;
    save(&path, &settings).expect("save");
    assert_eq!(load(&path).expect("load"), settings);
    let _ = fs::remove_dir_all(&dir);
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
fn watch_folders_load_back_and_an_empty_list_is_not_written() {
    let dir = scratch("watch-folders");
    let path = dir.join("settings.toml");
    let settings = AppSettings::default();
    save(&path, &settings).expect("save");
    let written = fs::read_to_string(&path).expect("written");
    assert!(!written.contains("watch_folders"), "{written}");
    assert_eq!(load(&path).expect("load"), settings);
    let settings = AppSettings {
        watch_folders: vec![PathBuf::from("/media/videos"), PathBuf::from("/gone/drive")],
        ..AppSettings::default()
    };
    save(&path, &settings).expect("save");
    assert_eq!(load(&path).expect("load"), settings);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_file_without_watch_folders_watches_none() {
    let settings = parse("cut_score = 25.0\n").expect("parse");
    assert!(settings.watch_folders.is_empty());
    let settings = parse("watch_folders = [\"/media/videos\"]\n").expect("parse");
    assert_eq!(settings.watch_folders, [PathBuf::from("/media/videos")]);
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
