//! The job's work directory: the paths of its database, its large files and its logs, and the
//! job id.
//!
//! **Role:** name every path of a job (`job.redb`, `job.lock`, `audio/`, `visual/`, `logs/`,
//! `report.md`, `sheet.txt`) in one place, write files so a killed job never leaves half of one
//! (Fix It's call cache is JSON), hold the job's database through `store`, and keep the owner's
//! corrections in it through `corrections`.
//!
//! **Position:** used by every other module of the crate and by the worker tasks.
//!
//! **Signals and state:** creates folders and files under the job's folder only; `store` keeps a
//! process-wide registry of the open job databases.
//!
//! **Invariants:** every file written here goes to `<name>.part` and is renamed, so a file that
//! exists is complete; the job id depends only on the video's path.

use std::fs;
use std::path::{Path, PathBuf};

use job_model::StepName;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::error::{Context, Result};

pub(crate) mod corrections;
pub mod store;

pub use corrections::{
    corrections_digest, put_fix_record, read_corrections, read_fix_record, read_text_corrections,
    update_corrections, update_text_corrections,
};
pub use store::{JobStore, StoredJob, load_job_record, load_step_records, read_job, read_stored};

/// One job's folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkDir {
    root: PathBuf,
}

impl WorkDir {
    pub fn new(root: impl Into<PathBuf>) -> WorkDir {
        WorkDir { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn at(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// The job database, owned by the one process that has it open.
    pub fn database(&self) -> PathBuf {
        self.at("job.redb")
    }
    /// The pid of the process that has the job database open, written once the open succeeds.
    pub fn lock(&self) -> PathBuf {
        self.at("job.lock")
    }
    pub fn mix(&self) -> PathBuf {
        self.at("audio/mix_16k.f32")
    }
    pub fn vocals(&self) -> PathBuf {
        self.at("audio/vocals_16k.f32")
    }
    pub fn background(&self) -> PathBuf {
        self.at("audio/background_16k.f32")
    }
    pub fn sheet_text(&self) -> PathBuf {
        self.at("sheet.txt")
    }
    /// Fix It's answered model calls, kept until a run finishes.
    pub fn fix_calls(&self) -> PathBuf {
        self.at("fix/calls")
    }
    /// Erase masks and the original pixels behind each plate, relative to the job folder.
    pub fn masks_relative() -> &'static str {
        "visual/masks"
    }
    /// Inpainted plates, relative to the job folder.
    pub fn plates_relative() -> &'static str {
        "visual/plates"
    }
    /// Composed lettering patches and review previews, relative to the job folder.
    pub fn patches_relative() -> &'static str {
        "visual/patches"
    }
    pub fn report(&self) -> PathBuf {
        self.at("report.md")
    }
    /// The empty folder the `claude` CLI runs in.
    pub fn claude_cwd(&self) -> PathBuf {
        self.at("claude-cwd")
    }
    /// Where a replaced subtitle file is kept.
    pub fn backup(&self) -> PathBuf {
        self.at("backup")
    }
    /// A worker's stderr.
    pub fn log(&self, step: StepName) -> PathBuf {
        self.at(&format!("logs/{step}.log"))
    }
}

/// The folder name of the job for `video`: its file stem as a slug, and 8 hex digits of its path.
pub fn job_id(video: &Path) -> String {
    let stem = video
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut slug = String::new();
    for c in stem.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    let digest = Sha256::digest(video.as_os_str().as_encoded_bytes());
    let hash: String = digest.iter().take(4).map(|b| format!("{b:02x}")).collect();
    if slug.is_empty() {
        hash
    } else {
        format!("{slug}-{hash}")
    }
}

/// The default folder of all work directories: `~/.local/share/tbd-subtitles/work`.
pub fn default_root() -> Result<PathBuf> {
    Ok(inference::model_store::app_data_dir()
        .context("cannot find the data folder")?
        .join("work"))
}

/// The machine-wide GPU lock file: `~/.local/share/tbd-subtitles/gpu.lock`.
pub fn gpu_lock_path() -> Result<PathBuf> {
    Ok(inference::model_store::app_data_dir()
        .context("cannot find the data folder")?
        .join("gpu.lock"))
}

/// Read a JSON file.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).context(format!("cannot read {}", path.display()))?;
    serde_json::from_str(&text).context(format!("cannot parse {}", path.display()))
}

/// Write a JSON file through a part file.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let text =
        serde_json::to_string_pretty(value).context(format!("cannot encode {}", path.display()))?;
    write_text(path, &text)
}

/// Write a text file through a part file, creating its folder.
pub fn write_text(path: &Path, text: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context(format!("cannot create {}", parent.display()))?;
    }
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    fs::write(&part, text).context(format!("cannot write {}", part.display()))?;
    fs::rename(&part, path).context(format!("cannot rename {}", part.display()))
}

/// Flush a file a step wrote to the disk, so the record that names it never commits before it.
pub fn sync_file(path: &Path) -> Result<()> {
    fs::File::open(path)
        .and_then(|file| file.sync_all())
        .context(format!("cannot sync {}", path.display()))
}

#[cfg(test)]
#[path = "tests/work_dir.rs"]
mod tests;
