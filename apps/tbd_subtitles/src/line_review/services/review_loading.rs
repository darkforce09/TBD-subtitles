//! A finished job's lines for review, read from its work directory: what each engine heard, what
//! the language model settled on, why the quality check flagged it, and the owner's corrections.
//!
//! **Role:** join `sheet.json`, the re-decodes, `adjudicated.json`, `qc.json`, `review.json` and
//! `probe.json` into a `ReviewSession`.
//!
//! **Position:** called by the application when the owner opens a job's review and after a
//! review run ends.
//!
//! **Signals and state:** reads the work directory; writes nothing.
//!
//! **Invariants:** an utterance appears once, in sheet order; a missing re-decode or correction
//! file means none, a missing sheet or adjudication is an error naming the file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use job_model::outputs::{AdjudicationPass, Corrections, ProbeDecoded, Redecode, Utterance};
use job_model::report::QcReport;

use crate::line_review::models::session::{Hypothesis, ReviewLine, ReviewSession};

/// The review of the job of `video` whose work directory is `work_dir`.
pub(crate) fn load(video: &Path, work_dir: &Path) -> Result<ReviewSession, String> {
    let sheet: Vec<Utterance> = read(&work_dir.join("sheet.json"))?;
    let adjudicated: AdjudicationPass = read(&work_dir.join("adjudicated.json"))?;
    let qc: QcReport = read(&work_dir.join("qc.json")).unwrap_or_default();
    let corrections: Corrections = optional(&work_dir.join("review.json"))?.unwrap_or_default();
    let probe: Option<ProbeDecoded> = optional(&work_dir.join("probe.json"))?;
    let mut again: HashMap<String, Vec<Hypothesis>> = HashMap::new();
    for (engine, tag) in [("parakeet", "ALT p"), ("whisper", "ALT w")] {
        let path = work_dir
            .join("adjudication")
            .join(format!("redecode_{engine}.json"));
        if let Some(redecode) = optional::<Redecode>(&path)? {
            for (id, chunk) in redecode.ids.iter().zip(&redecode.transcript.chunks) {
                let text = chunk
                    .words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                again.entry(id.clone()).or_default().push(Hypothesis {
                    tag: tag.to_string(),
                    text,
                });
            }
        }
    }
    let settled: HashMap<&str, (&str, &[String])> = adjudicated
        .lines
        .iter()
        .map(|l| (l.id.as_str(), (l.t.as_str(), l.f.as_slice())))
        .collect();
    let mut reasons: HashMap<&str, Vec<String>> = HashMap::new();
    for finding in &qc.findings {
        if let Some(id) = &finding.utterance {
            reasons.entry(id.as_str()).or_default().push(format!(
                "{}: {}",
                finding.check.describe(),
                finding.detail
            ));
        }
    }
    let lines = sheet
        .iter()
        .map(|u| {
            let (text, flags) = settled.get(u.id.as_str()).copied().unwrap_or(("", &[]));
            let mut hypotheses: Vec<Hypothesis> = u
                .hypotheses
                .iter()
                .map(|(tag, words)| Hypothesis {
                    tag: tag.clone(),
                    text: words.join(" "),
                })
                .collect();
            hypotheses.extend(again.remove(&u.id).unwrap_or_default());
            ReviewLine {
                id: u.id.clone(),
                start_s: u.start_s,
                end_s: u.end_s,
                adjudicated: text.to_string(),
                flags: flags.to_vec(),
                hypotheses,
                reasons: reasons.remove(u.id.as_str()).unwrap_or_default(),
            }
        })
        .collect();
    let (audio_position, picture) = probe.map_or((0, (0, 0)), |p| {
        (
            p.track.audio_position,
            p.probe.video.map_or((0, 0), |v| (v.width, v.height)),
        )
    });
    Ok(ReviewSession {
        video: video.to_path_buf(),
        work_dir: work_dir.to_path_buf(),
        audio_position,
        picture,
        lines,
        corrections,
        show_all: false,
        draft: None,
        notice: None,
    })
}

/// The work directory of `video`'s job under `work_root`.
pub(crate) fn work_dir(video: &Path, work_root: &Path) -> Result<PathBuf, String> {
    let video = std::fs::canonicalize(video)
        .map_err(|e| format!("cannot find {}: {e}", video.display()))?;
    Ok(work_root.join(pipeline::work_dir::job_id(&video)))
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    optional(path)?.ok_or_else(|| format!("{} is missing", path.display()))
}

fn optional<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| format!("cannot parse {}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("cannot read {}: {e}", path.display())),
    }
}

#[cfg(test)]
#[path = "tests/review_loading.rs"]
mod tests;
