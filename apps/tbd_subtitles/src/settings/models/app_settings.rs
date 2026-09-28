//! The owner's settings, as kept in `settings.toml`: where models and work files live, which
//! engines run, which language model settles the text, and which subtitle format is written.
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
//! to a default.

use std::path::PathBuf;

use job_model::job::{OutputFormat, Separator, WhisperModel};
use serde::{Deserialize, Serialize};

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
    pub(crate) engines: Engines,
    pub(crate) language_model: LanguageModel,
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
            engines: Engines::default(),
            language_model: LanguageModel::default(),
        }
    }
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
