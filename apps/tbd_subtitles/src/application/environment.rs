//! Where the window finds its files and how its threads reach it: the real paths when it runs,
//! scratch paths in the tests, which never touch the owner's home.

use std::path::PathBuf;

use crate::core::background::Wake;
use crate::settings::services::settings_file;

/// The paths and the wake the application uses.
pub(crate) struct Environment {
    /// The settings file.
    pub(crate) settings_path: PathBuf,
    /// The runtime folder holding the CUDA libraries.
    pub(crate) runtime_dir: PathBuf,
    /// The running binary's folder, where the Whisper worker sits.
    pub(crate) exe_dir: Option<PathBuf>,
    /// Asks for a frame; threads call it after sending news.
    pub(crate) wake: Wake,
    /// Whether to start the machine checks and the size measure when the window opens.
    pub(crate) background: bool,
}

impl Environment {
    /// The owner's real files, with `wake` from the window's context.
    pub(crate) fn real(wake: Wake) -> anyhow::Result<Environment> {
        Ok(Environment {
            settings_path: settings_file::default_path()?,
            runtime_dir: inference::model_store::runtime_dir()?,
            exe_dir: std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(PathBuf::from)),
            wake,
            background: true,
        })
    }

    /// Files under `root` only, and no background work.
    #[cfg(test)]
    pub(crate) fn scratch(root: &std::path::Path) -> Environment {
        Environment {
            settings_path: root.join("config").join("settings.toml"),
            runtime_dir: root.join("runtime"),
            exe_dir: None,
            wake: crate::core::background::no_wake(),
            background: false,
        }
    }
}
