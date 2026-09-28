//! A finished job's lines worth a listen and its problems, counted from its quality check, the
//! owner's corrections and the lines Fix It answered.
//!
//! **Role:** count the distinct lines of each group and in all, the lines the owner checked, the
//! lines Claude checked, the pass rules the job breaks, and the findings Fix It would ask about.
//!
//! **Position:** called by `report_loading` when a report or a row's summary is read; the
//! problems also by `fix_result`, from the problems before the first Fix It run.
//!
//! **Signals and state:** none; reads the check, the corrections and what Fix It answered.
//!
//! **Invariants:** a line is counted once however many findings name it, and once in each group
//! it has findings in; a finding about no line (a sound cue, the whole job) is no line to check;
//! a line the owner corrected is worth a listen and checked by the owner; a line Fix It changed is
//! worth a listen and checked by Claude until the owner keeps or undoes it, even once a correction
//! run has settled its findings, so the counts hold across the run; a flagged line the owner has
//! not checked whose every finding Fix It answered is checked by Claude; no line is checked twice;
//! the problems are empty exactly when the job passes (`QcReport::passes`), one per failed rule.

use std::collections::{BTreeMap, BTreeSet};

use job_model::outputs::Corrections;
use job_model::report::{CPS_TARGET, QcCheck, QcReport};
use stages::fix_it::items::{self, Answered};

use crate::job_report::models::finding_group::LineGroup;
use crate::job_report::models::problem::Problem;
use crate::job_report::models::summary::{LineCounts, RowSummary};

/// The lines worth a listen: those `qc` flags, in their groups; those the owner settled in
/// `corrections`, which are checked; and those Fix It changed and the owner has not checked, in
/// the Changed by Claude group and checked by Claude, as are the flagged lines whose every
/// finding `answered` covers. A corrected line stays counted after a correction run settles its
/// findings, then under no group.
pub(crate) fn line_counts(
    qc: &QcReport,
    corrections: &Corrections,
    answered: &Answered,
) -> LineCounts {
    let mut flagged: BTreeSet<&str> = BTreeSet::new();
    let mut groups: BTreeMap<LineGroup, BTreeSet<&str>> = BTreeMap::new();
    let mut open: BTreeSet<&str> = BTreeSet::new();
    for finding in &qc.findings {
        let (Some(group), Some(line)) =
            (LineGroup::of(finding.check), finding.utterance.as_deref())
        else {
            continue;
        };
        flagged.insert(line);
        groups.entry(group).or_default().insert(line);
        if !answered.covers(finding) {
            open.insert(line);
        }
    }
    let checked: BTreeSet<&str> = corrections
        .lines
        .iter()
        .filter(|line| line.by_owner())
        .map(|line| line.id.as_str())
        .collect();
    let fixed: BTreeSet<&str> = corrections
        .lines
        .iter()
        .filter(|line| !line.by_owner())
        .map(|line| line.id.as_str())
        .collect();
    let settled = flagged
        .iter()
        .filter(|line| !open.contains(*line) && !checked.contains(*line));
    let by_claude: BTreeSet<&str> = fixed.iter().chain(settled).copied().collect();
    flagged.extend(&checked);
    flagged.extend(&fixed);
    if !fixed.is_empty() {
        groups.insert(LineGroup::ChangedByFixIt, fixed);
    }
    LineCounts {
        flagged: flagged.len(),
        checked: checked.len(),
        by_claude: by_claude.len(),
        groups: LineGroup::ALL
            .iter()
            .filter_map(|group| groups.get(group).map(|lines| (*group, lines.len())))
            .collect(),
    }
}

/// How many of `qc`'s findings Fix It would ask about, the owner's `corrections` and the findings
/// `answered` covers left out.
pub(crate) fn fixable(qc: &QcReport, corrections: &Corrections, answered: &Answered) -> usize {
    qc.findings
        .iter()
        .filter(|finding| items::asks_about(finding, corrections, answered).is_some())
        .count()
}

/// The pass rules `qc` breaks, one problem each, in the order the file card lists them.
pub(crate) fn problems(qc: &QcReport) -> Vec<Problem> {
    let first_uncovered_s = qc
        .findings
        .iter()
        .filter(|finding| finding.check == QcCheck::UncoveredSpeech)
        .map(|finding| finding.time_s)
        .reduce(f64::min);
    problems_of(
        &qc.counts(),
        first_uncovered_s,
        qc.summary.cps_ok_share,
        qc.summary.cues,
    )
}

/// The pass rules broken by a check with `counts` findings per check, its first speech with no
/// subtitle at `first_uncovered_s`, `cps_ok_share` of its `cues` easy to read; one problem each,
/// in the order the file card lists them.
pub(crate) fn problems_of(
    counts: &BTreeMap<QcCheck, usize>,
    first_uncovered_s: Option<f64>,
    cps_ok_share: f64,
    cues: usize,
) -> Vec<Problem> {
    let count = |check: QcCheck| counts.get(&check).copied().unwrap_or(0);
    let layout: usize = counts
        .iter()
        .filter(|(check, _)| check.is_layout_violation())
        .map(|(_, n)| n)
        .sum();
    let mut problems = Vec::new();
    if layout > 0 {
        problems.push(Problem::Layout(layout));
    }
    if let Some(first) = first_uncovered_s {
        problems.push(Problem::UncoveredSpeech(first));
    }
    if count(QcCheck::Offset) > 0 {
        problems.push(Problem::Offset);
    }
    let failed = count(QcCheck::FailedCall);
    if failed > 0 {
        problems.push(Problem::FailedCall(failed));
    }
    if cues > 0 && cps_ok_share < CPS_TARGET {
        problems.push(Problem::ReadingSpeed(cps_ok_share));
    }
    problems
}

/// What the sidebar row of a job with `qc` and `corrections` says; `answered` holds the lines a
/// Fix It run of the job as it stands answered, and `fixed_by_claude` whether there is any.
pub(crate) fn summary(
    qc: &QcReport,
    corrections: &Corrections,
    answered: &Answered,
    fixed_by_claude: bool,
) -> RowSummary {
    let lines = line_counts(qc, corrections, answered);
    RowSummary {
        problems: problems(qc).len(),
        flagged: lines.flagged,
        to_check: lines.to_check(),
        fixed_by_claude,
    }
}

#[cfg(test)]
#[path = "tests/line_counts.rs"]
mod tests;
