//! The owner's settings, as kept in `settings.toml`: where models and work files live, which
//! engines run, which language model settles the text, which subtitle format is written, and
//! which folders are watched for videos.
//!
//! **Role:** hold every setting with its default, so a missing file or a missing key means the
//! measured defaults.
//!
//! **Position:** read and written by `settings::services::settings_file`; turned into a job's
//! settings by `settings::services::job_settings`; edited by the settings view; read by the
//! `process` subcommand.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** an unknown key is an error, never ignored, so a typo cannot silently fall back
//! to a default; a missing `onscreen_text.localized_video` means on, as for new jobs.

use std::path::PathBuf;

use job_model::job::{OutputFormat, Separator, WhisperModel};
use job_model::onscreen::TextSettings;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// The glossary name that means the built-in One Piece glossary.
pub(crate) const ONE_PIECE: &str = "one_piece";
/// The glossary name that means no glossary.
pub(crate) const NO_GLOSSARY: &str = "none";

/// Every setting the window edits and the command line reads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct AppSettings {
    /// The models folder; `None` is `~/.local/share/tbd-subtitles/models`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) models_dir: Option<PathBuf>,
    /// The folder of the jobs' work directories; `None` is `~/.local/share/tbd-subtitles/work`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) work_root: Option<PathBuf>,
    /// `one_piece` (built in), `none`, or the path of a JSON array of names.
    pub(crate) glossary: String,
    /// The lowest scdet score that counts as a shot cut.
    pub(crate) cut_score: f64,
    /// The subtitle file written beside the video.
    pub(crate) output_format: OutputFormat,
    /// The folders watched, with their subfolders, while the app is open: a video in one that has
    /// no subtitles yet is queued once it has finished downloading. An empty list is not written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) watch_folders: Vec<PathBuf>,
    pub(crate) engines: Engines,
    pub(crate) language_model: LanguageModel,
    /// How Japanese writing is translated and included in each new video's ASS subtitles, and
    /// whether it is replaced in a localized copy of the video.
    #[serde(deserialize_with = "saved_onscreen_text")]
    pub(crate) onscreen_text: TextSettings,
}

/// The model each engine step runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Engines {
    pub(crate) separator: Separator,
    pub(crate) whisper: WhisperModel,
}

/// The program that settles the text and fixes flagged lines: the backend, the model a run asks,
/// the model Fix It asks, how many run at once in a run and in Fix It, and whether Fix It follows
/// each run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LanguageModel {
    pub(crate) backend: Backend,
    /// The model name the backend is asked for, such as `sonnet`.
    pub(crate) model: String,
    /// The model Fix It asks, such as `opus`: a stronger one than the run's, since it reads the
    /// whole video and fixes only the flagged lines.
    pub(crate) fix_model: String,
    /// How many backend processes run at once.
    pub(crate) processes: usize,
    /// How many `claude` calls Fix It makes at once across every video it fixes.
    pub(crate) fix_calls: usize,
    /// Whether Fix It starts on each video when its full run finishes.
    pub(crate) fix_after_run: bool,
}

/// The language-model backends the app runs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Backend {
    /// The headless `claude` CLI, `claude -p` with a JSON schema.
    #[default]
    Claude,
}

impl Default for AppSettings {
    fn default() -> AppSettings {
        AppSettings {
            models_dir: None,
            work_root: None,
            glossary: ONE_PIECE.to_string(),
            cut_score: 20.0,
            output_format: OutputFormat::Srt,
            watch_folders: Vec::new(),
            engines: Engines::default(),
            language_model: LanguageModel::default(),
            onscreen_text: new_job_onscreen_text(),
        }
    }
}

/// On-screen text settings for new jobs: translated, and replaced in a localized video.
fn new_job_onscreen_text() -> TextSettings {
    TextSettings {
        localized_video: true,
        ..TextSettings::new_job()
    }
}

/// On-screen text settings as saved: a file written before the localized video existed has no
/// `localized_video` key and takes the new-job choice, on; a saved `false` stays off.
fn saved_onscreen_text<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<TextSettings, D::Error> {
    let table = toml::Table::deserialize(deserializer)?;
    let chosen = table.contains_key("localized_video");
    let mut settings: TextSettings = toml::Value::Table(table)
        .try_into()
        .map_err(D::Error::custom)?;
    if !chosen {
        settings.localized_video = true;
    }
    Ok(settings)
}

impl Default for Engines {
    fn default() -> Engines {
        Engines {
            separator: Separator::Roformer,
            whisper: WhisperModel::LargeV3,
        }
    }
}

impl Default for LanguageModel {
    fn default() -> LanguageModel {
        LanguageModel {
            backend: Backend::Claude,
            model: "sonnet".to_string(),
            fix_model: "opus".to_string(),
            processes: 8,
            fix_calls: 32,
            fix_after_run: false,
        }
    }
}
