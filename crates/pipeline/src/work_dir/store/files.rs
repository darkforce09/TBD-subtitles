//! The files outside `job.redb` that its records name, and the removal of every file in a
//! step-owned folder no record names.
//!
//! **Role:** list the files each step's stored rows name: the audio streams of the probe and the
//! separation, the crops and keyframe stills of the on-screen text documents, the masks, plates,
//! patches and previews of the replacement documents, the subtitle files beside the video and the
//! localized video; tell whether they are all there; and, when a job's database opens, remove the
//! files in the step-owned folders that nothing names.
//!
//! **Position:** in `work_dir::store`; used by `resume::is_valid` and by `JobStore::open`.
//!
//! **Signals and state:** reads rows in place from one snapshot; the cleanup removes files under
//! `audio/` and `visual/{crops,keyframes,masks,plates,patches}` only.
//!
//! **Invariants:** a document's reference is relative to the job folder and made of plain names,
//! or it is malformed and names nothing; caches (`visual/readings/`, `visual/translations/`,
//! `claude-*.json`, `fix/calls/`), `logs/`, `report.md` and `sheet.txt` are never touched; a
//! file a step writes is named by the rows that step commits, so a file is an orphan only when
//! its step's rows are gone or never committed.

use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use job_model::StepName;
use job_model::onscreen::{
    LocalizedVideoRecord, ReplacementDocument, TextDocument, VerifiedReplacements,
};
use job_model::outputs::OutputRecord;
use worker_channel::address::Table;

use super::{JobStore, StoreRead, keys};
use crate::error::Result;
use crate::work_dir::WorkDir;

/// The folders, relative to the job folder, that hold only files a step's rows name.
pub const OWNED_FOLDERS: [&str; 6] = [
    "audio",
    "visual/crops",
    "visual/keyframes",
    "visual/masks",
    "visual/plates",
    "visual/patches",
];

/// The files one step's rows name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamedFiles {
    /// Files in the job folder, as full paths; each must be a non-empty file.
    pub in_job: Vec<PathBuf>,
    /// Files outside the job folder (beside the video); each must be a file.
    pub beside: Vec<PathBuf>,
    /// A reference that is not a plain relative path, or a detection that names no crop for an
    /// occurrence: the rows cannot be resumed from.
    pub malformed: bool,
}

impl NamedFiles {
    /// Whether every named file is there and no reference is malformed.
    pub fn present(&self) -> bool {
        !self.malformed
            && self.in_job.iter().all(|path| {
                fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
            })
            && self.beside.iter().all(|path| path.is_file())
    }

    fn job_reference(&mut self, work: &WorkDir, reference: &str) {
        match in_job(work, reference) {
            Some(path) => self.in_job.push(path),
            None => self.malformed = true,
        }
    }
}

/// `reference` as a path in the job folder, or `None` when it is not a plain relative path.
fn in_job(work: &WorkDir, reference: &str) -> Option<PathBuf> {
    let path = Path::new(reference);
    let mut plain = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(name) => plain.push(name),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!plain.as_os_str().is_empty()).then(|| work.root().join(plain))
}

/// The files `step`'s rows in `read` name. A step whose document is missing names only its fixed
/// files: the probe's mix and the separation's stems.
pub fn named_files(step: StepName, read: &StoreRead, work: &WorkDir) -> Result<NamedFiles> {
    let mut named = NamedFiles::default();
    let key = keys::output_key(step, None);
    match step {
        StepName::ProbeDecode => named.in_job.push(work.mix()),
        StepName::Separation => named.in_job.extend([work.vocals(), work.background()]),
        StepName::TextDetect
        | StepName::TextRead
        | StepName::TextTrack
        | StepName::TextTranslate
        | StepName::TextReview
        | StepName::TextTypeset => {
            let references = read.view::<TextDocument, _>(Table::Outputs, &key, |document| {
                let mut references = Vec::new();
                let mut without_crop = false;
                for occurrence in document.occurrences.iter() {
                    without_crop |= occurrence.crops.is_empty();
                    references.extend(occurrence.crops.iter().map(|crop| crop.as_str().to_owned()));
                    if let Some(keyframe) = occurrence.keyframe.as_ref() {
                        references.push(keyframe.image.as_str().to_owned());
                    }
                }
                (references, without_crop)
            })?;
            if let Some((references, without_crop)) = references {
                named.malformed |= step == StepName::TextDetect && without_crop;
                for reference in references {
                    named.job_reference(work, &reference);
                }
            }
        }
        StepName::TextMask | StepName::TextInpaint | StepName::TextCompose => {
            let references =
                read.view::<ReplacementDocument, _>(Table::Outputs, &key, |document| {
                    replacement_references(document)
                })?;
            for reference in references.unwrap_or_default() {
                named.job_reference(work, &reference);
            }
        }
        StepName::TextVerify => {
            let references =
                read.view::<VerifiedReplacements, _>(Table::Outputs, &key, |verified| {
                    replacement_references(&verified.document)
                })?;
            for reference in references.unwrap_or_default() {
                named.job_reference(work, &reference);
            }
        }
        StepName::Output => {
            let paths = read.view::<OutputRecord, _>(Table::Outputs, &key, |record| {
                std::iter::once(record.path.as_str().to_owned())
                    .chain(
                        record
                            .localized
                            .as_ref()
                            .map(|path| path.as_str().to_owned()),
                    )
                    .collect::<Vec<_>>()
            })?;
            named
                .beside
                .extend(paths.unwrap_or_default().into_iter().map(PathBuf::from));
        }
        StepName::LocalizedVideo => {
            let path = read.view::<LocalizedVideoRecord, _>(Table::Outputs, &key, |record| {
                record.path.as_ref().map(|path| path.as_str().to_owned())
            })?;
            named.beside.extend(path.flatten().map(PathBuf::from));
        }
        _ => {}
    }
    Ok(named)
}

/// Every file a replacement document names: each plate's source, mask, fill and patch, and each
/// occurrence's review preview.
fn replacement_references(
    document: &<ReplacementDocument as rkyv::Archive>::Archived,
) -> Vec<String> {
    let mut references = Vec::new();
    for text in document.texts.iter() {
        for plate in text.plates.iter() {
            references.push(plate.source.as_str().to_owned());
            references.push(plate.mask.as_str().to_owned());
            references.extend(plate.plate.as_ref().map(|path| path.as_str().to_owned()));
            references.extend(plate.patch.as_ref().map(|path| path.as_str().to_owned()));
        }
        references.extend(text.preview.as_ref().map(|path| path.as_str().to_owned()));
    }
    references
}

/// Whether `step` has rows in `read`: its record or one of its documents.
fn has_rows(step: StepName, read: &StoreRead) -> Result<bool> {
    if read.step_record(step)?.is_some() {
        return Ok(true);
    }
    for key in keys::output_keys(step) {
        if read.raw(Table::Outputs, &key)?.is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Every file in the job folder that some step's rows name.
fn named_in_job(read: &StoreRead, work: &WorkDir) -> Result<HashSet<PathBuf>> {
    let mut named = HashSet::new();
    for step in StepName::ALL {
        if has_rows(step, read)? {
            named.extend(named_files(step, read, work)?.in_job);
        }
    }
    Ok(named)
}

/// Remove every file under the owned folders of `store`'s job that no row names; the files it
/// removed.
pub fn remove_orphans(store: &JobStore) -> Result<Vec<PathBuf>> {
    let work = store.work();
    let named = named_in_job(&store.read()?, work)?;
    let mut removed = Vec::new();
    for folder in OWNED_FOLDERS {
        let mut found = Vec::new();
        collect_files(&work.root().join(folder), &mut found);
        for file in found {
            if !named.contains(&file) && fs::remove_file(&file).is_ok() {
                removed.push(file);
            }
        }
    }
    if !removed.is_empty() {
        tracing::info!(
            "removed {} files of {} that no record names",
            removed.len(),
            work.root().display()
        );
    }
    Ok(removed)
}

/// Every file under `folder`, recursively; symbolic links count as files and are not followed.
fn collect_files(folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => collect_files(&path, found),
            Ok(_) => found.push(path),
            Err(_) => {}
        }
    }
}

#[cfg(test)]
#[path = "tests/files.rs"]
mod tests;
