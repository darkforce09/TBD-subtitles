//! The job's work directory: where every step's output lives, the job id, and JSON written so a
//! killed job never leaves half a file.
//!
//! **Role:** name every path of a job (`job.json`, `audio/`, `asr/`, `adjudication/`, `logs/`,
//! `steps/`, the outputs) in one place, and read and write the JSON files.
//!
//! **Position:** used by every other module of the crate and by the worker tasks.
//!
//! **Signals and state:** creates folders and files under the job's folder only.
//!
//! **Invariants:** every JSON file is written to `<name>.part` and renamed, so a file that exists
//! is complete; the job id depends only on the video's path.

use std::fs;
use std::path::{Path, PathBuf};

use job_model::StepName;
use serde::Serialize;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};

use crate::error::{Context, Result};

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

    pub fn job_json(&self) -> PathBuf {
        self.at("job.json")
    }
    pub fn lock(&self) -> PathBuf {
        self.at("job.lock")
    }
    pub fn probe(&self) -> PathBuf {
        self.at("probe.json")
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
    pub fn shots(&self) -> PathBuf {
        self.at("shots.json")
    }
    pub fn vad(&self) -> PathBuf {
        self.at("vad.json")
    }
    pub fn asr(&self, engine: &str) -> PathBuf {
        self.at(&format!("asr/{engine}.json"))
    }
    pub fn sheet(&self) -> PathBuf {
        self.at("sheet.json")
    }
    pub fn sheet_text(&self) -> PathBuf {
        self.at("sheet.txt")
    }
    pub fn sound_events(&self) -> PathBuf {
        self.at("sound_events.json")
    }
    pub fn first_pass(&self) -> PathBuf {
        self.at("adjudication/first.json")
    }
    pub fn redecode(&self, engine: &str) -> PathBuf {
        self.at(&format!("adjudication/redecode_{engine}.json"))
    }
    pub fn adjudicated(&self) -> PathBuf {
        self.at("adjudicated.json")
    }
    pub fn sound_cues(&self) -> PathBuf {
        self.at("sound_cues.json")
    }
    pub fn aligned(&self) -> PathBuf {
        self.at("aligned.json")
    }
    /// The owner's corrections, written by the window.
    pub fn review(&self) -> PathBuf {
        self.at("review.json")
    }
    /// The aligned words with the corrected lines timed again.
    pub fn reviewed(&self) -> PathBuf {
        self.at("reviewed.json")
    }
    pub fn cues(&self) -> PathBuf {
        self.at("cues.json")
    }
    pub fn dropped_sounds(&self) -> PathBuf {
        self.at("cues_dropped_sounds.json")
    }
    pub fn qc(&self) -> PathBuf {
        self.at("qc.json")
    }
    pub fn report(&self) -> PathBuf {
        self.at("report.md")
    }
    pub fn output_record(&self) -> PathBuf {
        self.at("output.json")
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
    /// What a worker reports about itself.
    pub fn worker_measure(&self, step: StepName) -> PathBuf {
        self.at(&format!("steps/{step}.worker.json"))
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

#[cfg(test)]
#[path = "tests/work_dir.rs"]
mod tests;
