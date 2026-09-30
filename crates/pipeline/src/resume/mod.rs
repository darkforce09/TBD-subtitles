//! Resume: a step's fingerprint, and whether its recorded output can be reused.
//!
//! **Role:** a fingerprint hashes the step's name and revision, the settings it reads, the
//! video's identity (for steps that read the video), and the fingerprint and finish time of each
//! step it reads. A step is reused when the record holds the same fingerprint and every output
//! file exists.
//!
//! **Position:** used by `runner` before each step.
//!
//! **Signals and state:** reads the job's corrections, the model files and the reference folder
//! for the fingerprints; writes nothing.
//!
//! **Invariants:** re-running a step changes its finish time, so every step that reads it runs
//! again; a missing output re-runs its step.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::TextCorrections;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::graph;
use crate::work_dir::WorkDir;

/// The fingerprint `step` would have if it ran now with this record.
pub fn fingerprint(step: StepName, record: &JobRecord) -> String {
    fingerprint_with_work(step, record, None)
}

/// A fingerprint that also covers corrections stored in this job's work directory.
pub fn fingerprint_in_work(step: StepName, record: &JobRecord, work: &WorkDir) -> String {
    fingerprint_with_work(step, record, Some(work))
}

fn fingerprint_with_work(step: StepName, record: &JobRecord, work: Option<&WorkDir>) -> String {
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
    if record.settings.onscreen_text.enabled {
        if matches!(
            step,
            StepName::TextDetect | StepName::TextRead | StepName::TextTranslate
        ) || (record.settings.onscreen_text.localized_video
            && matches!(
                step,
                StepName::TextInpaint | StepName::TextCompose | StepName::TextVerify
            ))
        {
            value["text_models"] = text_models(step, record);
        }
        if step == StepName::TextTranslate {
            value["text_references"] = text_references(record);
        }
        if matches!(
            step,
            StepName::TextRead | StepName::TextTranslate | StepName::TextReview
        ) {
            value["text_corrections"] = text_corrections(step, work);
        }
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
    done.fingerprint == fingerprint_in_work(step, record, work)
        && graph::artifacts_valid(
            step,
            work,
            Path::new(&record.video),
            record.settings.effective_output_format(),
        )
}

fn text_models(step: StepName, record: &JobRecord) -> Value {
    use inference::model_store::{MODEL_FILES, models_dir};

    let root = record
        .models_dir
        .as_ref()
        .map(PathBuf::from)
        .or_else(|| models_dir().ok());
    let files: Vec<_> = MODEL_FILES.iter().filter(|file| match step {
        StepName::TextDetect => file.model == "pp-ocrv5" && file.file.starts_with("det"),
        StepName::TextRead => file.model == "manga-ocr"
            || (file.model == "pp-ocrv5" && !file.file.starts_with("det")),
        StepName::TextTranslate => file.model == "qwen3.5-4b",
        StepName::TextInpaint => file.model == "lama-inpaint",
        StepName::TextCompose => file.model == "latin-fonts",
        StepName::TextVerify => file.model == "pp-ocrv5",
        _ => false,
    }).map(|file| {
        let path = root.as_ref().map(|root| root.join(file.model).join(file.file));
        let metadata = path.as_ref().and_then(|path| fs::metadata(path).ok());
        json!({
            "model": file.model, "file": file.file, "sha256": file.sha256,
            "size": file.size, "path": path,
            "installed_size": metadata.as_ref().map(fs::Metadata::len),
            "modified_ns": metadata.and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos().to_string()),
        })
    }).collect();
    json!(files)
}

fn text_corrections(step: StepName, work: Option<&WorkDir>) -> Value {
    let corrections = match work.map(WorkDir::text_corrections) {
        None => TextCorrections::default(),
        Some(path) => match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<TextCorrections>(&bytes) {
                Ok(corrections) => corrections,
                Err(error) => return json!({"error": error.to_string(), "sha256": digest(&bytes)}),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                TextCorrections::default()
            }
            Err(error) => return json!({"error": error.to_string()}),
        },
    };
    if step == StepName::TextReview {
        json!(corrections)
    } else {
        let mut retry = corrections.retry;
        retry.sort();
        json!(retry)
    }
}

/// Hash the flat reference folder in path order, excluding the job's own generated ASS.
fn text_references(record: &JobRecord) -> Value {
    let Some(folder) = &record.settings.onscreen_text.reference_folder else {
        return Value::Null;
    };
    let entries = match fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) => return json!({"error": error.to_string()}),
    };
    let generated = Path::new(&record.video).with_extension("ass");
    let generated = fs::canonicalize(&generated).unwrap_or(generated);
    let mut paths = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => return json!({"error": error.to_string()}),
        };
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|ext| ext.as_encoded_bytes().eq_ignore_ascii_case(b"ass"))
            && path.is_file()
        {
            let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if canonical != generated {
                paths.push(path);
            }
        }
    }
    paths.sort();
    let files: Vec<_> = paths
        .into_iter()
        .map(|path| match inference::model_store::sha256_of(&path) {
            Ok(hash) => json!({"path": path, "sha256": hash}),
            Err(error) => json!({"path": path, "error": error.to_string()}),
        })
        .collect();
    json!(files)
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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

#[cfg(test)]
#[path = "tests/resume.rs"]
mod tests;
