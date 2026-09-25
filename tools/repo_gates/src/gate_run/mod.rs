//! The machinery every gate shares.
//!
//! **Role:** the operator's [`GateRequest`], the tree of files a gate treats as tracked
//! ([`tracked_tree`]), the `--path` scope ([`gate_scope`]), the repository regions
//! ([`path_regions`]), fenced-block recognition ([`markdown_fences`]), the file-by-file rule
//! runner ([`file_rule`]), and [`GateRun`], which carries a gate's verdicts to
//! [`verification_core::Report`].
//!
//! **Position:** used by every gate under `crate::gates`, which `main` runs through
//! `crate::gates::run`. Every path a gate judges comes from `git ls-files` (the index, joined under
//! `--with-untracked` by the untracked files git does not ignore), and every region it applies
//! comes from [`crate::layout`].
//!
//! **Signals and state:** none held; a run lists the files once, judges them, prints its verdicts
//! and returns its exit status.
//!
//! **Invariants:** a gate that could not list the files, could not read a file it judges, or
//! whose scope selects nothing reports "did not run" (exit 2), never a pass; exit 1 means at
//! least one judged item broke a rule; exit 0 means every judged item held. A run that included
//! untracked files says so on its header and its summary line, so its result never passes for a
//! check of the committed files.

pub(crate) mod file_rule;
pub(crate) mod gate_scope;
pub(crate) mod markdown_fences;
pub(crate) mod path_regions;
pub(crate) mod tracked_tree;

#[cfg(test)]
#[path = "tests/fixture_checkout.rs"]
pub(crate) mod fixture_checkout;

use std::path::Path;

use verification_core::{Kind, NotRun, Report, Verdict};

use gate_scope::GateScope;
use tracked_tree::TrackedTree;
pub(crate) use tracked_tree::UntrackedFiles;

/// What the operator asked one documentation gate to judge.
#[derive(Debug, Default)]
pub(crate) struct GateRequest {
    /// The `--path` values as written; none means the whole repository.
    pub(crate) paths: Vec<String>,
    /// Whether the untracked files git does not ignore are judged like tracked ones
    /// (`--with-untracked`).
    pub(crate) untracked: UntrackedFiles,
}

/// What one run of a documentation gate concluded, in print order.
pub(crate) struct GateRun {
    /// The gate's name, on the header and, marked by [`GateRun::summary_label`], on the summary.
    pub(crate) label: &'static str,
    /// Whether the run's listing included untracked files.
    pub(crate) untracked: UntrackedFiles,
    /// Lines printed before the verdicts: what the gate judges and over which scope.
    pub(crate) header: Vec<String>,
    /// One verdict per judged item, in the order the gate judged them.
    pub(crate) verdicts: Vec<Verdict>,
    /// Lines printed after the verdicts: each rule's totals.
    pub(crate) totals: Vec<String>,
}

impl GateRun {
    /// A run with a header and nothing judged yet.
    pub(crate) fn new(
        label: &'static str,
        untracked: UntrackedFiles,
        header: Vec<String>,
    ) -> GateRun {
        GateRun {
            label,
            untracked,
            header,
            verdicts: Vec::new(),
            totals: Vec::new(),
        }
    }

    /// A run that judged nothing, carrying the one verdict that says why.
    pub(crate) fn stopped(
        label: &'static str,
        untracked: UntrackedFiles,
        header: Vec<String>,
        verdict: Verdict,
    ) -> GateRun {
        GateRun {
            verdicts: vec![verdict],
            ..GateRun::new(label, untracked, header)
        }
    }

    /// The name the summary line carries: the gate's own, followed by the flag when the run
    /// included untracked files, so a pasted summary never passes for a check of the committed
    /// files.
    pub(crate) fn summary_label(&self) -> String {
        match self.untracked {
            UntrackedFiles::Invisible => self.label.to_string(),
            UntrackedFiles::Included => {
                format!("{} --with-untracked (untracked files included)", self.label)
            }
        }
    }

    /// Print the header, every verdict through the shared report, the totals and the summary,
    /// and yield the exit status: 0 when every verdict held, 1 on a violation, 2 when any check
    /// did not run.
    pub(crate) fn print(self) -> u8 {
        for line in &self.header {
            println!("{line}");
        }
        let mut report = Report::new(self.summary_label());
        for verdict in self.verdicts {
            report.check(verdict);
        }
        for line in &self.totals {
            println!("{line}");
        }
        match report.finish() {
            0 => 0,
            1 => 1,
            _ => 2,
        }
    }
}

/// Running counts for one rule of a gate.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    /// Items the rule judged.
    pub(crate) judged: usize,
    /// Judged items that broke the rule.
    pub(crate) failed: usize,
    /// Judged items that could not be read.
    pub(crate) unread: usize,
}

impl Tally {
    /// Count one judged item by its verdict.
    pub(crate) fn count(&mut self, verdict: &Verdict) {
        self.judged += 1;
        match verdict {
            Verdict::Held => {}
            Verdict::Failed(_) => self.failed += 1,
            Verdict::DidNotRun(..) => self.unread += 1,
        }
    }
}

/// The header line that names a run's scope and how many files the listing held.
pub(crate) fn scope_line(scope: &GateScope, tree: &TrackedTree) -> String {
    let scope = scope.describe();
    let tracked = tree.tracked_file_count();
    match tree.untracked_file_count() {
        None => format!("    scope: {scope}; git listed {tracked} tracked file(s)"),
        Some(untracked) => format!(
            "    scope: {scope}; git listed {tracked} tracked file(s) and {untracked} untracked \
             file(s) it does not ignore"
        ),
    }
}

/// The tree and the resolved scope a gate judges, or the stopped run that says why the gate
/// cannot judge anything.
///
/// A listing that failed, a listing that holds no tracked file, and a `--path` value the tree
/// refuses are all "did not run": the gate never examined the files the operator asked about.
pub(crate) fn prepare(
    label: &'static str,
    kind: Kind,
    repo_root: &Path,
    listing: Result<TrackedTree, NotRun>,
    request: &GateRequest,
) -> Result<(TrackedTree, GateScope), GateRun> {
    let stopped = |verdict| {
        GateRun::stopped(
            label,
            request.untracked,
            vec![format!("==> {label}")],
            verdict,
        )
    };
    let tree = match listing {
        Ok(tree) if tree.tracked_file_count() == 0 => {
            let verdict = Verdict::did_not_run(
                format!("{label}: git listed no tracked file"),
                kind,
                NotRun::TargetMissing(repo_root.to_path_buf()),
            );
            return Err(stopped(verdict));
        }
        Ok(tree) => tree,
        Err(cause) => {
            let listed = match request.untracked {
                UntrackedFiles::Invisible => "the tracked files",
                UntrackedFiles::Included => "the tracked and untracked files",
            };
            let verdict =
                Verdict::did_not_run(format!("{label} could not list {listed}"), kind, cause);
            return Err(stopped(verdict));
        }
    };
    match GateScope::resolve(&request.paths, repo_root, &tree) {
        Ok(scope) => Ok((tree, scope)),
        Err(refusal) => {
            let verdict = Verdict::did_not_run(
                format!("{label} scope `{}` {}", refusal.value, refusal.reason),
                kind,
                NotRun::TargetMissing(repo_root.join(&refusal.value)),
            );
            Err(stopped(verdict))
        }
    }
}

/// The verdict for a scope in which a gate found nothing to judge: an empty judgement is never a
/// clean one.
pub(crate) fn judged_nothing(
    label: &str,
    kind: Kind,
    repo_root: &Path,
    scope: &GateScope,
) -> Verdict {
    Verdict::did_not_run(
        format!("{label} judged nothing in {}", scope.describe()),
        kind,
        NotRun::TargetMissing(scope.anchor(repo_root)),
    )
}

/// A tracked file's text. Bytes that are not UTF-8 are replaced rather than refused, so a stray
/// byte is judged instead of turning the check into "did not run"; a file git lists but the disk
/// lacks is [`NotRun::TargetMissing`].
pub(crate) fn read_tracked(repo_root: &Path, path: &str) -> Result<String, NotRun> {
    let full = repo_root.join(path);
    match std::fs::read(&full) {
        Ok(bytes) => Ok(String::from_utf8_lossy(&bytes).into_owned()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Err(NotRun::TargetMissing(full))
        }
        Err(source) => Err(NotRun::Unreadable { path: full, source }),
    }
}
