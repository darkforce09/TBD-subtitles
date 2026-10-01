//! A finished job's lines for review, read from its database: what each engine heard, what the
//! language model settled on, why the quality check flagged it, and the owner's corrections.
//!
//! **Role:** join the sheet, the re-decodes, the settled re-adjudication, the quality check, the
//! owner's corrections and the probe, all read from one snapshot of the job's database, into a
//! `ReviewSession`, each line with its groups and why in the owner's words.
//!
//! **Position:** called by the application when the owner opens a job's review and after a
//! review run ends.
//!
//! **Signals and state:** opens the job's database for one read (sharing this process's handle
//! while a job of it runs here); writes nothing.
//!
//! **Invariants:** an utterance appears once, in sheet order; a line Fix It changed that the owner
//! has not checked is in the Changed by Claude group, first, with what the app had and why; a
//! missing re-decode, quality check, correction or probe means none, a missing sheet or
//! adjudication is an error naming its row.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use job_model::StepName;
use job_model::outputs::{
    AdjudicationPass, Chosen, Corrections, ProbeDecoded, Redecode, Utterance,
};
use job_model::report::{QcCheck, QcFinding, QcReport};
use pipeline::work_dir::WorkDir;

use crate::job_report::models::finding_group::LineGroup;
use crate::line_review::models::session::{Hypothesis, ReviewLine, ReviewSession};

/// The review of the job of `video` whose work directory is `work_dir`.
pub(crate) fn load(video: &Path, work_dir: &Path) -> Result<ReviewSession, String> {
    let Stored {
        sheet,
        adjudicated,
        qc,
        corrections,
        probe,
        redecodes,
    } = stored(work_dir)?;
    let mut again: HashMap<String, Vec<Hypothesis>> = HashMap::new();
    for (redecode, tag) in redecodes.into_iter().zip(["ALT p", "ALT w"]) {
        if let Some(redecode) = redecode {
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
    for correction in &corrections.lines {
        if let Chosen::FixIt { why, .. } = &correction.chosen {
            let had = settled.get(correction.id.as_str()).map_or("", |(t, _)| *t);
            groups.entry(correction.id.as_str()).or_default().insert(
                LineGroup::ChangedByFixIt,
                format!("The app had “{had}”. {why}"),
            );
        }
    }
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

/// What the review reads from the job's database.
struct Stored {
    sheet: Vec<Utterance>,
    adjudicated: AdjudicationPass,
    qc: QcReport,
    corrections: Corrections,
    probe: Option<ProbeDecoded>,
    /// Parakeet's re-decode, then Whisper's.
    redecodes: [Option<Redecode>; 2],
}

/// The rows of the job in `work_dir` its review reads, in one snapshot; an error names the sheet
/// or the adjudication when either is missing, or the row that does not read.
fn stored(work_dir: &Path) -> Result<Stored, String> {
    let missing = |row: &str| {
        format!(
            "{} has no {row}",
            WorkDir::new(work_dir).database().display()
        )
    };
    let stored = pipeline::work_dir::read_stored(work_dir, |read| {
        Ok((
            read.output::<Vec<Utterance>>(StepName::DiffSheet, None)?,
            read.output::<AdjudicationPass>(StepName::Readjudicate, None)?,
            read.output::<QcReport>(StepName::Qc, None)
                .ok()
                .flatten()
                .unwrap_or_default(),
            read.line_corrections()?,
            read.output::<ProbeDecoded>(StepName::ProbeDecode, None)?,
            [
                read.output::<Redecode>(StepName::RedecodeParakeet, None)?,
                read.output::<Redecode>(StepName::RedecodeWhisper, None)?,
            ],
        ))
    })
    .map_err(|e| e.to_string())?
    .ok_or_else(|| missing("outputs/diff_sheet"))?;
    let (sheet, adjudicated, qc, corrections, probe, redecodes) = stored;
    Ok(Stored {
        sheet: sheet.ok_or_else(|| missing("outputs/diff_sheet"))?,
        adjudicated: adjudicated.ok_or_else(|| missing("outputs/readjudicate"))?,
        qc,
        corrections,
        probe,
        redecodes,
    })
}

#[cfg(test)]
#[path = "tests/review_loading.rs"]
mod tests;
