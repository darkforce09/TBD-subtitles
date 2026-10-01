//! Resume: a step's fingerprint, and whether its stored output can be reused.
//!
//! **Role:** a fingerprint hashes the step's name and revision, the settings it reads, the
//! video's identity (for steps that read the video), the layout version of every table, the
//! fingerprint and finish time of each step it reads, the owner's corrections it reads, and, for
//! the translation and the composition, the library signs their occurrences match, when any do. A
//! step is reused when its stored record holds the same fingerprint, every document it writes is
//! stored and every file its rows name is there.
//!
//! **Position:** used by `runner` before each step; reads through one `StoreRead` snapshot of the
//! job's database and `work_dir::store::files` for the files rows name.
//!
//! **Signals and state:** reads the job's step records, table layouts and corrections, the model
//! files, the reference folder and the sign library for the fingerprints; writes nothing.
//!
//! **Invariants:** re-running a step changes its finish time, so every step that reads it runs
//! again; a missing document or file re-runs its step; a row that does not read makes the step
//! stale, never valid.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::{TextCorrections, TextDocument};
use job_model::store::TableLayouts;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use worker_channel::address::Table;

use crate::error::Result;
use crate::graph;
use crate::library::{Library, signs};
use crate::work_dir::corrections::{digest_in, read_row};
use crate::work_dir::store::{StoreRead, files, keys};
use crate::work_dir::{self, WorkDir};

/// The fingerprint `step` would have if it ran now with this record and the rows of `read`.
pub fn fingerprint(
    step: StepName,
    record: &JobRecord,
    read: &StoreRead,
    library: Option<&Library>,
) -> Result<String> {
    let mut inputs = Vec::new();
    for input in graph::inputs(step) {
        let done = read.step_record(*input)?;
        inputs.push(json!({
            "step": input,
            "fingerprint": done.as_ref().map(|d| d.fingerprint.clone()),
            "finished_ns": done.as_ref().map(|d| d.finished_ns.to_string()),
        }));
    }
    let reads_video = graph::inputs(step).is_empty();
    let identity =
        reads_video.then(|| json!([record.video, record.video_size, record.video_modified_s]));
    let layouts = read
        .get::<TableLayouts>(Table::Meta, &keys::named(keys::LAYOUT))?
        .unwrap_or_default();
    let mut value = json!({
        "step": step,
        "revision": graph::revision(step),
        "settings": graph::settings(step, &record.settings),
        "video": identity,
        "layouts": layouts.versions,
        "inputs": inputs,
    });
    // Only the steps that read the corrections carry the key, so no other fingerprint changes.
    if graph::reads_corrections(step) {
        value["corrections"] = json!(digest_in(read, keys::LINE_CORRECTIONS)?);
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
        if graph::reads_text_corrections(step) {
            let corrections: TextCorrections = read_row(read, keys::TEXT_CORRECTIONS)?;
            value["text_corrections"] = text_corrections(step, corrections);
        }
        if let Some(library) = library
            && let Some(digest) = library_digest(step, record, read, library)?
        {
            value["library"] = json!(digest);
        }
    }
    Ok(digest(value.to_string().as_bytes()))
}

/// Whether the step's stored output is still good: its record holds the current fingerprint,
/// every document it writes is stored and every file its rows name is there.
pub fn is_valid(
    step: StepName,
    record: &JobRecord,
    read: &StoreRead,
    work: &WorkDir,
    library: Option<&Library>,
) -> bool {
    let Ok(Some(done)) = read.step_record(step) else {
        return false;
    };
    let current = fingerprint(step, record, read, library);
    let documents = keys::output_keys(step)
        .iter()
        .all(|key| read.raw(Table::Outputs, key).is_ok_and(|row| row.is_some()));
    current.is_ok_and(|current| current == done.fingerprint)
        && documents
        && files::named_files(step, read, work).is_ok_and(|named| named.present())
}

/// The digest of the library signs a step starts from: the translation's over the tracked
/// occurrences, the composition's over the reviewed ones; `None` for every other step, and when
/// no occurrence matches a sign of another job.
fn library_digest(
    step: StepName,
    record: &JobRecord,
    read: &StoreRead,
    library: &Library,
) -> Result<Option<String>> {
    let text = &record.settings.onscreen_text;
    let document = match step {
        StepName::TextTranslate => StepName::TextTrack,
        StepName::TextCompose if text.localized_video => StepName::TextReview,
        _ => return Ok(None),
    };
    let Some(document) = read.output::<TextDocument>(document, None)? else {
        return Ok(None);
    };
    let job = work_dir::job_id(Path::new(&record.video));
    let matches = signs::matches(library, &document, read.job_folder(), &job)?;
    Ok(signs::digest(&matches))
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

/// What of the text corrections a step's fingerprint covers: all of them for the review, only the
/// occurrences to retry for reading and translation.
fn text_corrections(step: StepName, corrections: TextCorrections) -> Value {
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

/// The steps a run would do now, in order: each step whose stored output is not valid, and each
/// step that reads one of them, because its fingerprint changes once that input runs again.
pub fn stale_steps(
    record: &JobRecord,
    read: &StoreRead,
    work: &WorkDir,
    library: Option<&Library>,
) -> Vec<StepName> {
    let mut stale: Vec<StepName> = Vec::new();
    for step in StepName::ALL {
        if !is_valid(step, record, read, work, library)
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
