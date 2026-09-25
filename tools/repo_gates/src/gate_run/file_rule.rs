//! One rule judged file by file over the tracked files a scope selects.
//!
//! **Role:** the shared shape of every gate that judges one file at a time: pick the tracked
//! files the rule selects, read each one's bytes, ask the rule for its problems, and turn them
//! into one verdict per file plus a totals line.
//!
//! **Position:** called by the gates under `crate::gates::source` and `crate::gates::workspace`
//! and by the README-section and status-line gates; uses [`super::prepare`] for the listing and
//! the scope.
//!
//! **Signals and state:** reads the selected files; holds nothing between runs.
//!
//! **Invariants:** a file that cannot be read is "did not run", never a pass; a scope in which
//! the rule selects nothing is "did not run"; a file holds exactly when the rule found no problem
//! in it.

use std::path::Path;

use verification_core::{Finding, Kind, NotRun, Verdict};

use super::tracked_tree::TrackedTree;
use super::{GateRequest, GateRun, Tally, judged_nothing, prepare, scope_line};

/// A rule judged file by file.
pub(crate) trait FileRule {
    /// The gate's name on its header and summary lines.
    fn gate(&self) -> &'static str;
    /// What the rule holds, for the header line.
    fn describes(&self) -> String;
    /// Whether the rule judges the tracked file at `path`.
    fn selects(&self, path: &str) -> bool;
    /// Every problem the rule finds in the file at `path` holding `bytes`, one line each.
    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String>;
}

/// `cargo gates <gate>` for a file rule: judge the files `request` lists at `repo_root`, print
/// every verdict, and return the exit status.
pub(crate) fn verify_file_rule(rule: &dyn FileRule, repo_root: &Path, request: &GateRequest) -> u8 {
    judge_file_rule(
        rule,
        repo_root,
        TrackedTree::load(repo_root, request.untracked),
        request,
    )
    .print()
}

/// The rule over an already-attempted listing, so a failed listing and a fixture tree take the
/// same path as a real run.
pub(crate) fn judge_file_rule(
    rule: &dyn FileRule,
    repo_root: &Path,
    listing: Result<TrackedTree, NotRun>,
    request: &GateRequest,
) -> GateRun {
    let gate = rule.gate();
    let (tree, scope) = match prepare(gate, Kind::Ban, repo_root, listing, request) {
        Ok(prepared) => prepared,
        Err(stopped) => return stopped,
    };
    let mut run = GateRun::new(
        gate,
        request.untracked,
        vec![
            format!("==> {gate}: {}", rule.describes()),
            scope_line(&scope, &tree),
        ],
    );
    let mut tally = Tally::default();
    for path in tree
        .files()
        .filter(|path| scope.contains(path) && rule.selects(path))
    {
        let verdict = match read_bytes(repo_root, path) {
            Err(cause) => {
                Verdict::did_not_run(format!("{path} could not be read"), Kind::Ban, cause)
            }
            Ok(bytes) => {
                let problems = rule.problems(path, &bytes);
                if problems.is_empty() {
                    Verdict::Held
                } else {
                    Verdict::Failed(Finding {
                        headline: format!("{path}: {} problem(s)", problems.len()),
                        detail: problems,
                    })
                }
            }
        };
        tally.count(&verdict);
        run.verdicts.push(verdict);
    }
    if tally.judged == 0 {
        run.verdicts
            .push(judged_nothing(gate, Kind::Ban, repo_root, &scope));
        return run;
    }
    run.totals = vec![format!(
        "  {} file(s) judged, {} with problems, {} unreadable",
        tally.judged, tally.failed, tally.unread
    )];
    run
}

/// A listed file's bytes; a file git lists but the disk lacks is [`NotRun::TargetMissing`].
fn read_bytes(repo_root: &Path, path: &str) -> Result<Vec<u8>, NotRun> {
    let full = repo_root.join(path);
    match std::fs::read(&full) {
        Ok(bytes) => Ok(bytes),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Err(NotRun::TargetMissing(full))
        }
        Err(source) => Err(NotRun::Unreadable { path: full, source }),
    }
}

/// Whether `path` is a test source: any of its folders is named `tests`.
pub(crate) fn is_test_path(path: &str) -> bool {
    path.split('/')
        .rev()
        .skip(1)
        .any(|folder| folder == "tests")
}

/// The extension of `path`'s file name, if it has one after a non-empty stem.
pub(crate) fn extension(path: &str) -> Option<&str> {
    let name = super::path_regions::file_name(path);
    name.rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, extension)| extension)
}

#[cfg(test)]
#[path = "tests/file_rule.rs"]
mod tests;
