//! The models folder and the runtime folder: pinned downloads, checked before use.
//!
//! **Role:** name where model files and runtime libraries live, download the pinned ones that
//! are missing, and tell a caller whether a model is complete on disk.
//!
//! **Position:** called by the stack spike tool's `fetch` command and, later, by the app's model
//! download; `cuda_runtime` reads the runtime folder it fills. Uses `manifest.rs`, `download.rs`
//! and `archive.rs`.
//!
//! **Signals and state:** reads `XDG_DATA_HOME` and `HOME`; writes under
//! `<data home>/tbd-subtitles/models/` and `<data home>/tbd-subtitles/runtime/`.
//!
//! **Invariants:** nothing is converted; a file is used only when its size matches its pin, and
//! only a download whose SHA-256 matched is ever renamed into place.

mod archive;
mod download;
pub mod manifest;

use std::fmt;
use std::path::{Path, PathBuf};

pub use archive::{install as install_archive, strip_first};
pub use download::{Progress, fetch_verified, hex, sha256_of};
pub use manifest::{
    CUDA_ARCHIVES, MODEL_FILES, ONNX_RUNTIME_ARCHIVE, PinnedArchive, PinnedFile, runtime_archives,
};

/// Why a download or a check failed.
#[derive(Debug)]
pub enum StoreError {
    /// Neither `XDG_DATA_HOME` nor `HOME` is set.
    NoDataHome,
    /// A model id the manifest does not hold.
    UnknownModel(String),
    Io {
        path: PathBuf,
        message: String,
    },
    Http {
        url: String,
        message: String,
    },
    SizeMismatch {
        path: PathBuf,
        expected: u64,
        found: u64,
    },
    ChecksumMismatch {
        path: PathBuf,
        expected: String,
        found: String,
    },
    Archive {
        path: PathBuf,
        message: String,
    },
}

impl StoreError {
    pub(crate) fn io(path: &Path, e: std::io::Error) -> StoreError {
        StoreError::Io {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::NoDataHome => write!(f, "neither XDG_DATA_HOME nor HOME is set"),
            StoreError::UnknownModel(id) => write!(f, "no model named {id} in the manifest"),
            StoreError::Io { path, message } => write!(f, "{}: {message}", path.display()),
            StoreError::Http { url, message } => write!(f, "{url}: {message}"),
            StoreError::SizeMismatch {
                path,
                expected,
                found,
            } => write!(f, "{}: {found} bytes, expected {expected}", path.display()),
            StoreError::ChecksumMismatch {
                path,
                expected,
                found,
            } => write!(
                f,
                "{}: SHA-256 {found}, expected {expected}",
                path.display()
            ),
            StoreError::Archive { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for StoreError {}

/// `<data home>/tbd-subtitles`, where data home is `XDG_DATA_HOME` or `~/.local/share`.
pub fn app_data_dir() -> Result<PathBuf, StoreError> {
    let base = match std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        Some(dir) => PathBuf::from(dir),
        None => {
            let home = std::env::var_os("HOME").ok_or(StoreError::NoDataHome)?;
            PathBuf::from(home).join(".local").join("share")
        }
    };
    Ok(base.join("tbd-subtitles"))
}

/// The default models folder.
pub fn models_dir() -> Result<PathBuf, StoreError> {
    Ok(app_data_dir()?.join("models"))
}

/// The default runtime folder holding the CUDA libraries.
pub fn runtime_dir() -> Result<PathBuf, StoreError> {
    Ok(app_data_dir()?.join("runtime"))
}

/// The folder of one model inside `models`.
pub fn model_dir(models: &Path, model: &str) -> Result<PathBuf, StoreError> {
    if manifest::files_of(model).next().is_none() {
        return Err(StoreError::UnknownModel(model.to_string()));
    }
    Ok(models.join(model))
}

/// Whether every file of `model` is in place with its pinned size.
pub fn is_complete(models: &Path, model: &str) -> bool {
    manifest::files_of(model).all(|file| {
        std::fs::metadata(models.join(file.model).join(file.file))
            .is_ok_and(|m| m.len() == file.size)
    })
}

/// Download every missing file of `model` into `models`.
pub fn fetch_model(
    models: &Path,
    model: &str,
    progress: &mut dyn FnMut(&PinnedFile, u64, u64),
) -> Result<PathBuf, StoreError> {
    let dir = model_dir(models, model)?;
    for file in manifest::files_of(model) {
        let dest = dir.join(file.file);
        fetch_verified(
            file.url,
            &dest,
            file.size,
            file.sha256,
            &mut |held, total| progress(file, held, total),
        )?;
    }
    Ok(dir)
}

#[cfg(test)]
#[path = "tests/model_store.rs"]
mod tests;
