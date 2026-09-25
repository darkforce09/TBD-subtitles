//! The spike's inputs and the files its items share in the work folder.
//!
//! **Role:** hold the video, the work folder and the FFmpeg programs, and read and write the JSON
//! files items pass to each other.
//!
//! **Position:** built by `main.rs` for every command; passed to each item.
//!
//! **Signals and state:** reads `XDG_DATA_HOME` or `HOME` for the default work folder; writes
//! only inside the work folder.
//!
//! **Invariants:** a JSON file is written to `<name>.part` and renamed, so it is whole or absent.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use inference::model_store;
use job_model::outputs::ProbeResult;
use media_io::Programs;

/// Everything an item needs to find its inputs and write its outputs.
pub(crate) struct Context {
    pub(crate) video: PathBuf,
    pub(crate) work: PathBuf,
    pub(crate) programs: Programs,
}

impl Context {
    pub(crate) fn new(video: &Path, work: Option<&Path>) -> anyhow::Result<Context> {
        let work = match work {
            Some(dir) => dir.to_path_buf(),
            None => default_work_dir(video)?,
        };
        std::fs::create_dir_all(work.join("results"))
            .with_context(|| format!("creating {}", work.display()))?;
        Ok(Context {
            video: video.to_path_buf(),
            work,
            programs: Programs::default(),
        })
    }

    /// A file in the work folder.
    pub(crate) fn path(&self, name: &str) -> PathBuf {
        self.work.join(name)
    }

    /// The probe result written by the decode item.
    pub(crate) fn probe(&self) -> anyhow::Result<ProbeResult> {
        let path = self.path("probe.json");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("{} is missing: run the decode item first", path.display()))?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Write a JSON output, whole or not at all.
    pub(crate) fn write_json<T: serde::Serialize>(
        &self,
        name: &str,
        value: &T,
    ) -> anyhow::Result<()> {
        let path = self.path(name);
        let part = self.path(&format!("{name}.part"));
        std::fs::write(&part, serde_json::to_vec_pretty(value)?)?;
        std::fs::rename(&part, &path)?;
        Ok(())
    }
}

/// `<data home>/tbd-subtitles/work/spike-<video name as a slug>`.
fn default_work_dir(video: &Path) -> anyhow::Result<PathBuf> {
    let stem = video
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "video".to_string());
    let slug: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    Ok(model_store::app_data_dir()?
        .join("work")
        .join(format!("spike-{slug}")))
}
