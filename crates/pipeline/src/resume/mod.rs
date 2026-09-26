//! Resume: a step's fingerprint, whether its recorded output can be reused, and the lock that
//! keeps two runs off one job.
//!
//! **Role:** a fingerprint hashes the step's name and revision, the settings it reads, the
//! video's identity (for steps that read the video), and the fingerprint and finish time of each
//! step it reads. A step is reused when the record holds the same fingerprint and every output
//! file exists.
//!
//! **Position:** used by `runner` before each step.
//!
//! **Signals and state:** the lock file holds the running process's pid.
//!
//! **Invariants:** re-running a step changes its finish time, so every step that reads it runs
//! again; a missing output re-runs its step; a lock whose pid is gone is taken over.

use std::fs;
use std::path::Path;

use job_model::StepName;
use job_model::job::JobRecord;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::error::{Context, PipelineError, Result};
use crate::graph;
use crate::work_dir::WorkDir;

/// The fingerprint `step` would have if it ran now with this record.
pub fn fingerprint(step: StepName, record: &JobRecord) -> String {
    let inputs: Vec<_> = graph::inputs(step)
        .iter()
        .map(|input| {
            let done = record.steps.get(input);
            json!({
                "step": input,
                "fingerprint": done.map(|d| d.fingerprint.clone()),
                "finished_ns": done.map(|d| d.finished_ns.to_string()),
            })
        })
        .collect();
    let reads_video = graph::inputs(step).is_empty();
    let identity =
        reads_video.then(|| json!([record.video, record.video_size, record.video_modified_s]));
    let mut value = json!({
        "step": step,
        "revision": graph::revision(step),
        "settings": graph::settings(step, &record.settings),
        "video": identity,
        "inputs": inputs,
    });
    // Only the steps that read the corrections carry the key, so no other fingerprint changes.
    if graph::reads_corrections(step) {
        value["corrections"] = json!(record.corrections);
    }
    let text = value.to_string();
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Whether the step's recorded output is still good: same fingerprint, every file there.
pub fn is_valid(step: StepName, record: &JobRecord, work: &WorkDir) -> bool {
    let Some(done) = record.steps.get(&step) else {
        return false;
    };
    done.fingerprint == fingerprint(step, record)
        && graph::outputs(
            step,
            work,
            Path::new(&record.video),
            record.settings.output_format,
        )
        .iter()
        .all(|p| p.exists())
}

/// The steps a run would do now, in order: each step whose record is not valid, and each step
/// that reads one of them, because its fingerprint changes once that input runs again.
pub fn stale_steps(record: &JobRecord, work: &WorkDir) -> Vec<StepName> {
    let mut stale: Vec<StepName> = Vec::new();
    for step in StepName::ALL {
        if !is_valid(step, record, work)
            || graph::inputs(step)
                .iter()
                .any(|input| stale.contains(input))
        {
            stale.push(step);
        }
    }
    stale
}

/// Holds a job's lock file while alive.
#[derive(Debug)]
pub struct JobLock {
    path: std::path::PathBuf,
}

impl Drop for JobLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Take the job's lock, refusing when another live process holds it.
pub fn lock(work: &WorkDir) -> Result<JobLock> {
    let path = work.lock();
    if let Ok(text) = fs::read_to_string(&path)
        && let Ok(pid) = text.trim().parse::<u32>()
        && pid != std::process::id()
        && Path::new(&format!("/proc/{pid}")).exists()
    {
        return Err(PipelineError::new(
            format!("job {}", work.root().display()),
            format!(
                "process {pid} is already running it (lock {})",
                path.display()
            ),
        ));
    }
    fs::create_dir_all(work.root()).context(format!("cannot create {}", work.root().display()))?;
    fs::write(&path, std::process::id().to_string())
        .context(format!("cannot write {}", path.display()))?;
    Ok(JobLock { path })
}

#[cfg(test)]
#[path = "tests/resume.rs"]
mod tests;
