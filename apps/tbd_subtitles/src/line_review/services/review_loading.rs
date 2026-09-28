//! A finished job's lines for review, read from its work directory: what each engine heard, what
//! the language model settled on, why the quality check flagged it, and the owner's corrections.
//!
//! **Role:** join `sheet.json`, the re-decodes, `adjudicated.json`, `qc.json`, `review.json` and
//! `probe.json` into a `ReviewSession`, each line with its groups and why in the owner's words.
//!
//! **Position:** called by the application when the owner opens a job's review and after a
//! review run ends.
//!
//! **Signals and state:** reads the work directory; writes nothing.
//!
//! **Invariants:** an utterance appears once, in sheet order; a missing re-decode or correction
//! file means none, a missing sheet or adjudication is an error naming the file.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use job_model::outputs::{AdjudicationPass, Corrections, ProbeDecoded, Redecode, Utterance};
use job_model::report::{QcCheck, QcFinding, QcReport};

use crate::job_report::models::finding_group::LineGroup;
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
    let mut groups: HashMap<&str, BTreeMap<LineGroup, String>> = HashMap::new();
    for finding in &qc.findings {
        if let (Some(id), Some(group)) = (&finding.utterance, LineGroup::of(finding.check)) {
            groups
                .entry(id.as_str())
                .or_default()
                .entry(group)
                .or_insert_with(|| why(finding));
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
                groups: groups
                    .remove(u.id.as_str())
                    .unwrap_or_default()
                    .into_iter()
                    .collect(),
            }
        })
        .collect();
    let mut session = ReviewSession::new(
        video.to_path_buf(),
        work_dir.to_path_buf(),
        lines,
        corrections,
    );
    if let Some(p) = probe {
        session.audio_position = p.track.audio_position;
        session.picture = p.probe.video.map_or((0, 0), |v| (v.width, v.height));
    }
    Ok(session)
}

/// Why `finding` put its line in its group, in the owner's words.
fn why(finding: &QcFinding) -> String {
    // `U0412: Frankie,` names the word after the line's id.
    let word = || {
        let detail = finding.detail.as_str();
        let word = detail.split_once(": ").map_or(detail, |(_, word)| word);
        word.trim_end_matches([',', '.', '!', '?']).to_string()
    };
    match finding.check {
        QcCheck::Unsure => "The engines disagreed and a second listen didn't settle it. The app's \
                            best guess is in the file."
            .to_string(),
        QcCheck::RemovedLocked => {
            format!(
                "Both engines heard “{}”; the subtitles don't use it.",
                word()
            )
        }
        QcCheck::Novel => format!("Neither engine heard “{}”.", word()),
        QcCheck::TooFast => format!(
            "{}; the limit is 20.",
            finding.detail.replace("cps", "characters per second")
        ),
        QcCheck::WeakTiming => "Fewer than half the words were timed by the aligner. Looks \
                                Right re-times it."
            .to_string(),
        check => format!("Layout: {}.", check.describe()),
    }
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
