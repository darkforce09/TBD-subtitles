use std::path::PathBuf;

use job_model::job::OutputFormat;

use super::*;
use crate::settings::models::app_settings::AppSettings;

fn page(name: &str) -> SettingsPage {
    let dir = std::env::temp_dir().join(format!("tbd-page-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    SettingsPage {
        path: dir.join("settings.toml"),
        saved: AppSettings::default(),
        draft: AppSettings::default(),
        notice: None,
        items: Vec::new(),
        download: None,
        checks: None,
        checking: false,
        work_folder: PathBuf::from("/work"),
        work_size: None,
    }
}

#[test]
fn a_saved_draft_is_written_and_becomes_the_settings() {
    let mut page = page("save");
    page.draft.output_format = OutputFormat::Ass;
    assert!(page.has_edits());
    save(&mut page);
    assert!(!page.has_edits());
    assert_eq!(page.saved.output_format, OutputFormat::Ass);
    assert_eq!(
        settings_file::load(&page.path).expect("load").output_format,
        OutputFormat::Ass
    );
    assert!(!page.notice.as_ref().expect("notice").is_error);
    let _ = std::fs::remove_dir_all(page.path.parent().expect("dir"));
}

#[test]
fn a_draft_that_cannot_make_a_job_is_not_saved() {
    let mut page = page("bad");
    page.draft.glossary = "/no/such/glossary.json".into();
    save(&mut page);
    assert!(page.has_edits());
    assert!(!page.path.exists());
    let notice = page.notice.expect("notice");
    assert!(notice.is_error && notice.text.contains("/no/such/glossary.json"));
}

#[test]
fn revert_throws_the_draft_away() {
    let mut page = page("revert");
    page.draft.cut_score = 40.0;
    revert(&mut page);
    assert!(!page.has_edits());
}
