//! A finished job's lines worth a listen and its problems, counted from its quality check and the
//! owner's corrections.
//!
//! **Role:** count the distinct lines of each group and in all, the lines the owner checked, and
//! the pass rules the job breaks; name the first line of a group.
//!
//! **Position:** called by `report_loading` when a report or a row's summary is read, and by the
//! application when Check Lines opens on a group.
//!
//! **Signals and state:** none; reads the check and the corrections.
//!
//! **Invariants:** a line is counted once however many findings name it, and once in each group
//! it has findings in; a finding about no line (a sound cue, the whole job) is no line to check;
//! a line the owner corrected is worth a listen and checked, even once a correction run has
//! settled its findings, so the counts hold across the run; the problems are empty exactly when
//! the job passes (`QcReport::passes`), one per failed rule.

use std::collections::{BTreeMap, BTreeSet};

use job_model::outputs::Corrections;
use job_model::report::{CPS_TARGET, QcCheck, QcReport};

use crate::job_report::models::finding_group::LineGroup;
use crate::job_report::models::problem::Problem;
use crate::job_report::models::summary::{LineCounts, RowSummary};

/// The lines worth a listen: those `qc` flags, in their groups, and those the owner corrected in
/// `corrections`, which are checked. A corrected line stays counted after a correction run
/// settles its findings, then under no group.
pub(crate) fn line_counts(qc: &QcReport, corrections: &Corrections) -> LineCounts {
    let mut flagged: BTreeSet<&str> = BTreeSet::new();
    let mut groups: BTreeMap<LineGroup, BTreeSet<&str>> = BTreeMap::new();
    for finding in &qc.findings {
        let (Some(group), Some(line)) =
            (LineGroup::of(finding.check), finding.utterance.as_deref())
        else {
            continue;
        };
        flagged.insert(line);
        groups.entry(group).or_default().insert(line);
    }
    let checked: BTreeSet<&str> = corrections
        .lines
        .iter()
        .map(|line| line.id.as_str())
        .collect();
    flagged.extend(&checked);
    LineCounts {
        flagged: flagged.len(),
        checked: checked.len(),
        groups: LineGroup::ALL
            .iter()
            .filter_map(|group| groups.get(group).map(|lines| (*group, lines.len())))
            .collect(),
    }
}

/// The pass rules `qc` breaks, one problem each, in the order the file card lists them.
pub(crate) fn problems(qc: &QcReport) -> Vec<Problem> {
    let counts = qc.counts();
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
    let uncovered = qc
        .findings
        .iter()
        .filter(|finding| finding.check == QcCheck::UncoveredSpeech)
        .map(|finding| finding.time_s)
        .reduce(f64::min);
    if let Some(first) = uncovered {
        problems.push(Problem::UncoveredSpeech(first));
    }
    if count(QcCheck::Offset) > 0 {
        problems.push(Problem::Offset);
    }
    let failed = count(QcCheck::FailedCall);
    if failed > 0 {
        problems.push(Problem::FailedCall(failed));
    }
    if qc.summary.cues > 0 && qc.summary.cps_ok_share < CPS_TARGET {
        problems.push(Problem::ReadingSpeed(qc.summary.cps_ok_share));
    }
    problems
}

/// What the sidebar row of a job with `qc` and `corrections` says.
pub(crate) fn summary(qc: &QcReport, corrections: &Corrections) -> RowSummary {
    let lines = line_counts(qc, corrections);
    RowSummary {
        problems: problems(qc).len(),
        flagged: lines.flagged,
        to_check: lines.to_check(),
    }
}

/// The earliest line with a finding of `group`.
pub(crate) fn first_line(qc: &QcReport, group: LineGroup) -> Option<String> {
    qc.findings
        .iter()
        .filter(|finding| LineGroup::of(finding.check) == Some(group))
        .find_map(|finding| finding.utterance.clone())
}

#[cfg(test)]
#[path = "tests/line_counts.rs"]
mod tests;
