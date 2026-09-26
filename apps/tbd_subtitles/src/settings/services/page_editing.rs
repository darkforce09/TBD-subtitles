//! Changing the settings page: saving the draft to the file after checking it can make a job,
//! and throwing it away.

use crate::settings::models::page::{Notice, SettingsPage};
use crate::settings::services::{job_settings, settings_file};

/// Save the draft: it must make a job's settings (its glossary readable), then it is written to
/// the settings file and becomes the saved settings. The notice says what happened.
pub(crate) fn save(page: &mut SettingsPage) {
    let checked = job_settings::job_settings(&page.draft)
        .map_err(|e| format!("{e:#}"))
        .and_then(|_| settings_file::save(&page.path, &page.draft).map_err(|e| e.to_string()));
    page.notice = Some(match checked {
        Ok(()) => {
            page.saved = page.draft.clone();
            Notice {
                text: format!("Saved to {}.", page.path.display()),
                is_error: false,
            }
        }
        Err(text) => Notice {
            text,
            is_error: true,
        },
    });
}

/// Throw the draft away.
pub(crate) fn revert(page: &mut SettingsPage) {
    page.draft = page.saved.clone();
    page.notice = None;
}

#[cfg(test)]
#[path = "tests/page_editing.rs"]
mod tests;
