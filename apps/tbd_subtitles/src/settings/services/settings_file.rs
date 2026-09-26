//! `settings.toml`: where it lives, reading it with every error named, and writing it whole.
//!
//! **Role:** turn the file's text into `AppSettings` and back.
//!
//! **Position:** called by the `process` subcommand and by the application when the window
//! starts and when the owner saves the settings view.
//!
//! **Signals and state:** reads `XDG_CONFIG_HOME` and `HOME`; reads and writes the settings file
//! (through a part file, renamed).
//!
//! **Invariants:** a missing file is the defaults; any other failure to read or parse is an error
//! naming the file and the key, never a silent default.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::settings::models::app_settings::AppSettings;

/// Why the settings could not be read or written.
#[derive(Debug)]
pub(crate) enum SettingsError {
    /// No `XDG_CONFIG_HOME` and no `HOME` to put the file under.
    NoConfigHome,
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Parse {
        path: PathBuf,
        message: String,
    },
    Write {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::NoConfigHome => {
                write!(f, "neither XDG_CONFIG_HOME nor HOME is set")
            }
            SettingsError::Io { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            SettingsError::Parse { path, message } => {
                write!(f, "{} is not valid settings: {message}", path.display())
            }
            SettingsError::Write { path, message } => {
                write!(f, "cannot write {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for SettingsError {}

/// `$XDG_CONFIG_HOME/tbd-subtitles/settings.toml`, or `~/.config/tbd-subtitles/settings.toml`.
pub(crate) fn default_path() -> Result<PathBuf, SettingsError> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map(|home| PathBuf::from(home).join(".config"))
        })
        .ok_or(SettingsError::NoConfigHome)?;
    Ok(config.join("tbd-subtitles").join("settings.toml"))
}

/// The settings in `path`; the defaults when there is no file.
pub(crate) fn load(path: &Path) -> Result<AppSettings, SettingsError> {
    match fs::read_to_string(path) {
        Ok(text) => parse(&text).map_err(|message| SettingsError::Parse {
            path: path.to_path_buf(),
            message,
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(AppSettings::default()),
        Err(source) => Err(SettingsError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// The settings a file's text holds; missing keys take their defaults.
pub(crate) fn parse(text: &str) -> Result<AppSettings, String> {
    toml::from_str(text).map_err(|e| e.to_string().trim_end().to_string())
}

/// The settings as the file's text.
pub(crate) fn render(settings: &AppSettings) -> Result<String, String> {
    toml::to_string_pretty(settings).map_err(|e| e.to_string())
}

/// Write `settings` to `path` whole: a part file, then a rename.
pub(crate) fn save(path: &Path, settings: &AppSettings) -> Result<(), SettingsError> {
    let error = |message: String| SettingsError::Write {
        path: path.to_path_buf(),
        message,
    };
    let text = render(settings).map_err(error)?;
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder).map_err(|e| error(e.to_string()))?;
    }
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    fs::write(&part, text).map_err(|e| error(e.to_string()))?;
    fs::rename(&part, path).map_err(|e| error(e.to_string()))
}

#[cfg(test)]
#[path = "tests/settings_file.rs"]
mod tests;
