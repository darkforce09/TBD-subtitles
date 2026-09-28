//! Changing a setting: an edit is written to the settings file at once, or refused with an error
//! under its field.
//!
//! **Role:** apply one edit of the Settings window to the page and the file (a new glossary is
//! read first, a number must be in its range, the models folder stays while a download runs), say
//! what the edit makes stale (the models list, a folder's size), keep a settings file that could
//! not be read before the first write replaces it, and read the saved glossary's names.
//!
//! **Position:** called by the application's settings actions for each edit and each chosen path,
//! and when the window opens; uses `job_settings` and `settings_file`.
//!
//! **Signals and state:** writes the settings file; renames an unreadable one to
//! `settings.toml.broken` beside it.
//!
//! **Invariants:** an edit that is refused or cannot be written leaves the file and `saved` as
//! they were; nothing out of range, and no glossary that cannot be read, is ever written; the
//! glossary is read only when it changes, so an unreadable one blocks no other edit and its error
//! stays under the glossary; the machine checks never go stale by an edit.

use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};

use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::page::{Field, FieldError, SettingsPage};
use crate::settings::services::{job_settings, settings_file};

/// How many language-model processes may run at once, and the shot cut scores allowed.
pub(crate) const PROCESSES: RangeInclusive<usize> = 1..=16;
pub(crate) const CUT_SCORES: RangeInclusive<f64> = 1.0..=100.0;

/// What a written edit makes stale.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Stale {
    /// The models and runtime archives the settings need: the models folder or an engine changed.
    pub(crate) models: bool,
    /// The models folder's size.
    pub(crate) models_size: bool,
    /// The work folder's size.
    pub(crate) work_size: bool,
}

/// What applying an edit did: what it made stale, and where a settings file that could not be
/// read was kept before the edit replaced it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Applied {
    pub(crate) stale: Stale,
    pub(crate) kept: Option<PathBuf>,
}

/// Apply `edited`, the saved settings with one field changed: written at once, or refused with an
/// error under that field when it is out of range, names a glossary that cannot be read, or moves
/// the models folder while a download runs.
pub(crate) fn apply(page: &mut SettingsPage, edited: AppSettings) -> Applied {
    let Some(field) = changed_field(&page.saved, &edited) else {
        return Applied::default();
    };
    let refuse = |page: &mut SettingsPage, message: String| {
        page.error = Some(FieldError { field, message });
    };
    if let Some(message) = out_of_bounds(page, field, &edited) {
        refuse(page, message);
        return Applied::default();
    }
    let names = if field == Field::Glossary {
        match job_settings::job_settings(&edited) {
            Ok(job) => Some(job.glossary.len()),
            Err(error) => {
                refuse(page, unusable_glossary(&edited.glossary, &error, true));
                return Applied::default();
            }
        }
    } else {
        None
    };
    let kept = match keep_unreadable(page) {
        Ok(kept) => kept,
        Err(message) => {
            refuse(page, message);
            return Applied::default();
        }
    };
    if let Err(error) = settings_file::save(&page.path, &edited) {
        let message = format!(
            "{}. The change was not saved.",
            capitalised(&error.to_string())
        );
        refuse(page, message);
        return Applied {
            stale: Stale::default(),
            kept,
        };
    }
    let stale = stale(&page.saved, &edited);
    page.saved = edited;
    page.unreadable = None;
    // An unreadable glossary's error stays until the glossary itself changes.
    page.error = page
        .error
        .take()
        .filter(|error| error.field == Field::Glossary && field != Field::Glossary);
    if names.is_some() {
        page.glossary_names = names;
    }
    Applied { stale, kept }
}

/// Why `edited` cannot be written before anything is read: a number out of its range, or the
/// models folder changed while a download fills it.
fn out_of_bounds(page: &SettingsPage, field: Field, edited: &AppSettings) -> Option<String> {
    let range = |what: &str, range: String| format!("{what} must be {range}. It was not changed.");
    match field {
        Field::ModelsFolder if page.download.is_some() => {
            Some("Stop the download first. The models folder was not changed.".to_string())
        }
        Field::Processes if !PROCESSES.contains(&edited.language_model.processes) => Some(range(
            "Processes at once",
            format!("{} to {}", PROCESSES.start(), PROCESSES.end()),
        )),
        Field::CutScore
            if !(edited.cut_score.is_finite() && CUT_SCORES.contains(&edited.cut_score)) =>
        {
            Some(range(
                "The shot cut score",
                format!(
                    "a number from {} to {}",
                    CUT_SCORES.start(),
                    CUT_SCORES.end()
                ),
            ))
        }
        _ => None,
    }
}

/// Before the first write over a settings file that could not be read, keep it beside itself as
/// `settings.toml.broken`; where it was kept, or why it could not be.
fn keep_unreadable(page: &SettingsPage) -> Result<Option<PathBuf>, String> {
    if page.unreadable.is_none() {
        return Ok(None);
    }
    let mut kept = page.path.as_os_str().to_owned();
    kept.push(".broken");
    let kept = PathBuf::from(kept);
    match std::fs::rename(&page.path, &kept) {
        Ok(()) => Ok(Some(kept)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!(
            "{} could not be kept as {}: {error}. The change was not saved.",
            page.path.display(),
            kept.display()
        )),
    }
}

/// Count the saved glossary's names, or say under the glossary why it cannot be read.
pub(crate) fn read_glossary(page: &mut SettingsPage) {
    match job_settings::job_settings(&page.saved) {
        Ok(job) => page.glossary_names = Some(job.glossary.len()),
        Err(error) => {
            page.glossary_names = None;
            page.error = Some(FieldError {
                field: Field::Glossary,
                message: unusable_glossary(&page.saved.glossary, &error, false),
            });
        }
    }
}

/// The first field that differs between `before` and `after`.
pub(crate) fn changed_field(before: &AppSettings, after: &AppSettings) -> Option<Field> {
    let (b, a) = (before, after);
    [
        (b.models_dir != a.models_dir, Field::ModelsFolder),
        (b.work_root != a.work_root, Field::WorkFolder),
        (b.output_format != a.output_format, Field::OutputFormat),
        (b.glossary != a.glossary, Field::Glossary),
        (b.engines.separator != a.engines.separator, Field::Separator),
        (b.engines.whisper != a.engines.whisper, Field::Whisper),
        (
            b.language_model.model != a.language_model.model,
            Field::Model,
        ),
        (
            b.language_model.fix_model != a.language_model.fix_model,
            Field::FixModel,
        ),
        (
            b.language_model.processes != a.language_model.processes,
            Field::Processes,
        ),
        (b.cut_score != a.cut_score, Field::CutScore),
    ]
    .into_iter()
    .find_map(|(changed, field)| changed.then_some(field))
}

/// What going from `before` to `after` makes stale.
pub(crate) fn stale(before: &AppSettings, after: &AppSettings) -> Stale {
    let models_folder = before.models_dir != after.models_dir;
    Stale {
        models: models_folder || before.engines != after.engines,
        models_size: models_folder,
        work_size: before.work_root != after.work_root,
    }
}

/// Why a glossary cannot be used: its file's name and the reason, and for an edit that it was not
/// changed.
fn unusable_glossary(glossary: &str, error: &anyhow::Error, edit: bool) -> String {
    let unchanged = if edit {
        " The glossary was not changed."
    } else {
        ""
    };
    format!(
        "Can't use {}: {}.{unchanged}",
        file_name(glossary),
        error.root_cause()
    )
}

/// The last part of a glossary's path.
fn file_name(glossary: &str) -> String {
    Path::new(glossary).file_name().map_or_else(
        || glossary.to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
#[path = "tests/page_editing.rs"]
mod tests;
