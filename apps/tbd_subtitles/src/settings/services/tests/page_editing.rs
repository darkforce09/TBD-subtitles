use std::path::PathBuf;

use job_model::job::{OutputFormat, WhisperModel};

use super::*;
use crate::settings::models::page::DownloadProgress;

fn page(name: &str) -> SettingsPage {
    let dir = std::env::temp_dir().join(format!("tbd-page-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    SettingsPage {
        path: dir.join("settings.toml"),
        saved: AppSettings::default(),
        error: None,
        unreadable: None,
        glossary_names: None,
        items: Vec::new(),
        download: None,
        downloaded_at: None,
        checks: None,
        checking: false,
        models_folder: PathBuf::from("/models"),
        models_size: None,
        work_folder: PathBuf::from("/work"),
        work_size: None,
    }
}

fn remove(page: &SettingsPage) {
    let _ = std::fs::remove_dir_all(page.path.parent().expect("dir"));
}

#[test]
fn a_valid_edit_is_written_at_once() {
    let mut page = page("valid");
    let mut edited = page.saved.clone();
    edited.output_format = OutputFormat::Ass;
    let applied = apply(&mut page, edited);
    assert_eq!(
        applied,
        Applied::default(),
        "a format needs nothing measured"
    );
    assert_eq!(page.saved.output_format, OutputFormat::Ass);
    assert_eq!(
        settings_file::load(&page.path).expect("load").output_format,
        OutputFormat::Ass
    );
    assert!(page.error.is_none());
    remove(&page);
}

#[test]
fn a_bad_glossary_is_not_written_and_names_its_field() {
    let mut page = page("bad");
    let mut edited = page.saved.clone();
    edited.glossary = "/no/such/names.json".into();
    assert_eq!(apply(&mut page, edited), Applied::default());
    assert!(!page.path.exists(), "nothing is written");
    assert_eq!(page.saved, AppSettings::default());
    let error = page.error.clone().expect("an error");
    assert_eq!(error.field, Field::Glossary);
    assert!(
        error.message.starts_with("Can't use names.json: ")
            && error.message.ends_with("The glossary was not changed."),
        "{}",
        error.message
    );
    let mut edited = page.saved.clone();
    edited.glossary = "none".into();
    apply(&mut page, edited);
    assert!(page.error.is_none(), "a glossary written clears its error");
    assert_eq!(page.glossary_names, Some(0));
    remove(&page);
}

#[test]
fn an_unreadable_saved_glossary_blocks_no_other_edit_and_its_error_stays() {
    let mut page = page("saved-bad");
    page.saved.glossary = "/no/such/names.json".into();
    read_glossary(&mut page);
    assert_eq!(page.glossary_names, None);
    let error = page.error.clone().expect("an error");
    assert_eq!(error.field, Field::Glossary);
    assert!(error.message.contains("names.json"), "{}", error.message);
    let mut edited = page.saved.clone();
    edited.engines.whisper = WhisperModel::LargeV3Turbo;
    assert!(apply(&mut page, edited).stale.models);
    assert_eq!(
        settings_file::load(&page.path)
            .expect("written")
            .engines
            .whisper,
        WhisperModel::LargeV3Turbo
    );
    assert_eq!(page.error, Some(error), "the glossary's error stays");
    remove(&page);
}

#[test]
fn numbers_out_of_range_or_not_finite_are_never_written() {
    let mut page = page("numbers");
    for processes in [0, 17] {
        let mut edited = page.saved.clone();
        edited.language_model.processes = processes;
        apply(&mut page, edited);
        assert_eq!(page.error.as_ref().map(|e| e.field), Some(Field::Processes));
    }
    for score in [f64::NAN, f64::INFINITY, 0.5, 101.0] {
        let mut edited = page.saved.clone();
        edited.cut_score = score;
        apply(&mut page, edited);
        assert_eq!(
            page.error.as_ref().map(|e| e.field),
            Some(Field::CutScore),
            "{score}"
        );
    }
    assert!(!page.path.exists(), "nothing invalid is written");
    let mut edited = page.saved.clone();
    edited.language_model.processes = 16;
    apply(&mut page, edited);
    assert_eq!(page.saved.language_model.processes, 16);
    assert!(page.error.is_none());
    remove(&page);
}

#[test]
fn the_models_folder_stays_while_a_download_runs() {
    let mut page = page("downloading");
    page.download = Some(DownloadProgress {
        id: "ced-base".into(),
        held: 0,
        total: 1,
        finished: 0,
        size: 1,
    });
    let mut edited = page.saved.clone();
    edited.models_dir = Some(PathBuf::from("/elsewhere"));
    apply(&mut page, edited);
    assert_eq!(page.saved.models_dir, None);
    let error = page.error.expect("an error");
    assert_eq!(error.field, Field::ModelsFolder);
    assert!(error.message.starts_with("Stop the download first."));
}

#[test]
fn an_unreadable_settings_file_is_kept_before_the_first_write() {
    let mut page = page("unreadable");
    std::fs::create_dir_all(page.path.parent().expect("dir")).expect("dir");
    std::fs::write(&page.path, "cut_score = [").expect("broken file");
    page.unreadable = Some("broken".into());
    let mut edited = page.saved.clone();
    edited.cut_score = 30.0;
    let applied = apply(&mut page, edited);
    let kept = page.path.with_extension("toml.broken");
    assert_eq!(applied.kept, Some(kept.clone()));
    assert_eq!(
        std::fs::read_to_string(&kept).expect("kept"),
        "cut_score = ["
    );
    assert_eq!(
        settings_file::load(&page.path).expect("written").cut_score,
        30.0
    );
    assert_eq!(page.unreadable, None, "the file is readable again");
    remove(&page);
}

#[test]
fn an_edit_that_changes_nothing_writes_nothing() {
    let mut page = page("same");
    let edited = page.saved.clone();
    assert_eq!(apply(&mut page, edited), Applied::default());
    assert!(!page.path.exists());
}

#[test]
fn only_the_models_folder_and_the_engines_make_the_models_list_stale() {
    let before = AppSettings::default();
    let mut whisper = before.clone();
    whisper.engines.whisper = WhisperModel::LargeV3Turbo;
    assert_eq!(
        stale(&before, &whisper),
        Stale {
            models: true,
            models_size: false,
            work_size: false
        }
    );
    let folder = AppSettings {
        models_dir: Some(PathBuf::from("/big/models")),
        ..before.clone()
    };
    assert_eq!(
        stale(&before, &folder),
        Stale {
            models: true,
            models_size: true,
            work_size: false
        }
    );
    let work = AppSettings {
        work_root: Some(PathBuf::from("/big/work")),
        ..before.clone()
    };
    assert_eq!(
        stale(&before, &work),
        Stale {
            models: false,
            models_size: false,
            work_size: true
        }
    );
    let mut model = before.clone();
    model.language_model.model = "opus".into();
    assert_eq!(stale(&before, &model), Stale::default());
    assert_eq!(changed_field(&before, &model), Some(Field::Model));
    let mut fix = before.clone();
    fix.language_model.fix_model = "fable".into();
    assert_eq!(stale(&before, &fix), Stale::default());
    assert_eq!(changed_field(&before, &fix), Some(Field::FixModel));
}
