//! A job's documents against the sign library: the signs its occurrences match, the digest the
//! fingerprints take of them, the styles the composition starts from, the signs a finished job
//! approves, and the signs a rejected occurrence removes.
//!
//! **Role:** hash each occurrence's keyframe crop outside any transaction, then look every
//! occurrence up in one; turn the replacements `text_verify` approved into `LibrarySign`s.
//!
//! **Position:** used by `resume::fingerprint`, by the `text_translate`, `text_compose` and
//! `output` tasks, and by the window's Check Text through [`forget`].
//!
//! **Signals and state:** reads the crops, patches and masks the documents name, relative to the
//! job folder; the library through `Library`.
//!
//! **Invariants:** a job never matches a sign that came from itself, so recording its own signs
//! never makes its own steps stale; an occurrence with no Japanese, no crop, or a crop that does
//! not decode matches nothing and is never recorded; an empty library hashes no crop; only a
//! replacement that stayed baked with every reading passed is approved.

use std::collections::BTreeMap;
use std::path::Path;

use job_model::onscreen::{
    LetteringStyle, LibrarySign, ReplaceStatus, ReplacementDocument, TextDocument, TextOccurrence,
    TextTreatment, VerifiedReplacements,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::Library;
use super::key::{crop_hash, normalised};
use crate::error::{Context, Result};

/// The library's sign for each matched occurrence, by occurrence id.
pub type Matches = BTreeMap<String, LibrarySign>;

/// The sign each occurrence of `document` matches: the same normalised Japanese, a crop hash
/// within `MATCH_DISTANCE`, from a job other than `job`. `root` is the job folder.
pub fn matches(
    library: &Library,
    document: &TextDocument,
    root: &Path,
    job: &str,
) -> Result<Matches> {
    if library.size()?.signs == 0 {
        return Ok(Matches::new());
    }
    let hashed: Vec<(&TextOccurrence, u64)> = document
        .occurrences
        .iter()
        .filter_map(|occurrence| Some((occurrence, hash_of(occurrence, root)?)))
        .collect();
    let wanted: Vec<(&str, u64)> = hashed
        .iter()
        .map(|(occurrence, hash)| (occurrence.japanese.as_str(), *hash))
        .collect();
    let found = library.lookup_all(&wanted, Some(job))?;
    Ok(hashed
        .iter()
        .zip(found)
        .filter_map(|((occurrence, _), sign)| Some((occurrence.id.clone(), sign?)))
        .collect())
}

/// What a fingerprint covers of `matches`: each occurrence's sign by its key, English,
/// confidence and style; `None` when nothing matched, so a job the library does not touch keeps
/// its fingerprints.
pub fn digest(matches: &Matches) -> Option<String> {
    if matches.is_empty() {
        return None;
    }
    let shown: Vec<_> = matches
        .iter()
        .map(|(id, sign)| {
            json!({
                "occurrence": id,
                "japanese": normalised(&sign.japanese),
                "crop_hash": format!("{:016x}", sign.crop_hash),
                "english": sign.english,
                "confidence": sign.confidence,
                "style": sign.style,
            })
        })
        .collect();
    let hash = Sha256::digest(json!(shown).to_string().as_bytes());
    Some(hash.iter().map(|b| format!("{b:02x}")).collect())
}

/// Give each pending replacement whose occurrence matched a sign that sign's lettering style.
pub fn start_from_styles(document: &mut ReplacementDocument, matches: &Matches) {
    for text in &mut document.texts {
        if text.status == ReplaceStatus::Pending
            && let Some(sign) = matches.get(&text.id)
        {
            text.style = Some(sign.style.clone());
        }
    }
}

/// The signs the finished job `job` approves: each replacement `text_verify` kept baked, with a
/// check whose every reading passed, of an occurrence of `text` with Japanese, English and a
/// crop that hashes, that the owner neither kept in Japanese nor moved to a nearby label. Its
/// patch and mask are those of its first plate; `root` is the job folder.
pub fn approved(
    verified: &VerifiedReplacements,
    text: &TextDocument,
    root: &Path,
    job: &str,
    now_s: u64,
) -> Result<Vec<LibrarySign>> {
    let occurrences: BTreeMap<&str, &TextOccurrence> = text
        .occurrences
        .iter()
        .map(|occurrence| (occurrence.id.as_str(), occurrence))
        .collect();
    let mut signs = Vec::new();
    for replaced in verified.document.baked() {
        let passed = verified
            .check(&replaced.id)
            .is_some_and(|check| check.samples > 0 && check.passed);
        let (Some(occurrence), Some(style), Some(plate)) = (
            occurrences.get(replaced.id.as_str()),
            replaced.style.as_ref(),
            replaced.plates.first(),
        ) else {
            continue;
        };
        let english = occurrence
            .english
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        let kept =
            occurrence.reviewed && occurrence.presentation.treatment == TextTreatment::Nearby;
        let Some(patch) = &plate.patch else {
            continue;
        };
        if !passed || kept || english.is_empty() {
            continue;
        }
        let Some(hash) = hash_of(occurrence, root) else {
            continue;
        };
        let read = |relative: &Path| {
            let path = root.join(relative);
            std::fs::read(&path).context(format!("cannot read {}", path.display()))
        };
        signs.push(LibrarySign {
            japanese: occurrence.japanese.clone(),
            crop_hash: hash,
            english: english.to_string(),
            confidence: if occurrence.reviewed {
                1.0
            } else {
                occurrence.confidence
            },
            style: LetteringStyle::clone(style),
            patch_png: read(patch)?,
            mask_png: read(&plate.mask)?,
            episodes: vec![job.to_string()],
            added_s: now_s,
        });
    }
    Ok(signs)
}

/// Remove the signs `occurrence` of the job in `root` matches, because the owner rejected or
/// changed its English; how many went.
pub fn forget(library: &Library, occurrence: &TextOccurrence, root: &Path) -> Result<usize> {
    match hash_of(occurrence, root) {
        Some(hash) => library.remove(&occurrence.japanese, hash),
        None => Ok(0),
    }
}

/// The crop hash of `occurrence`'s keyframe crop; `None` when it has no Japanese, no crop, or a
/// crop that does not decode.
fn hash_of(occurrence: &TextOccurrence, root: &Path) -> Option<u64> {
    if normalised(&occurrence.japanese).is_empty() {
        return None;
    }
    let crop = occurrence.crops.first()?;
    crop_hash(&root.join(crop)).ok()
}

#[cfg(test)]
#[path = "tests/signs.rs"]
mod tests;
