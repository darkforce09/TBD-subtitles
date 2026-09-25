//! README coverage: every folder in the README span carries a README.md whose Contents block
//! matches the folder.
//!
//! **Role:** the `readme-coverage` gate. Over every tracked folder of the README span (the code
//! trees and the documentation root, minus the exempt folders) that the scope selects, it judges
//! two rules: the folder carries a tracked README.md, and that README.md has a Contents block that
//! lists exactly the folder's tracked direct children.
//!
//! **Position:** `cargo gates readme-coverage [--path <dir>]... [--with-untracked]` calls
//! [`verify_readme_coverage`]. [`contents_block`] reads a README's Contents block,
//! [`entry_pattern`] matches an entry's name or glob, and [`folder_matching`] pairs the entries
//! with the children the [`TrackedTree`] lists.
//!
//! **Signals and state:** none; one pass over the tracked tree per run.
//!
//! **Invariants:** only the listed files count, as children and as READMEs: the tracked ones,
//! and under `--with-untracked` the untracked ones git does not ignore; both rules judge the same
//! folders, so an exempt folder (test, generated-output, hidden, pending-merge) and the README.md
//! it holds are judged by neither; every Contents violation prints as `path:line: message`; a
//! README that cannot be read is "did not run", never a pass.

mod contents_block;
mod entry_pattern;
mod folder_matching;

use std::path::Path;

use verification_core::{Finding, Kind, NotRun, Verdict};

use crate::gate_run::path_regions::{README, in_readme_span, join};
use crate::gate_run::tracked_tree::{FolderChildren, TrackedTree};
use crate::gate_run::{
    GateRequest, GateRun, Tally, judged_nothing, prepare, read_tracked, scope_line,
};
use crate::layout::{CODE_TREES, DOCUMENTATION_ROOT};

/// The gate's name on its header and summary lines.
const GATE: &str = "readme-coverage";

/// Whether a folder child or a Contents entry is a file or a folder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChildKind {
    File,
    Folder,
}

/// One way a README breaks the Contents grammar or fails to match its folder.
#[derive(Debug, PartialEq, Eq)]
struct Violation {
    /// The 1-based README line the violation concerns.
    line: usize,
    message: String,
}

/// `cargo gates readme-coverage`: judge the files `request` lists at `repo_root` over its
/// `--path` scope, print every verdict, and return the exit status (0 held, 1 violation, 2 did
/// not run).
pub(crate) fn verify_readme_coverage(repo_root: &Path, request: &GateRequest) -> u8 {
    judge(
        repo_root,
        TrackedTree::load(repo_root, request.untracked),
        request,
    )
    .print()
}

/// The gate over an already-attempted listing, so a failed listing and a fixture tree take the
/// same path as a real run.
fn judge(repo_root: &Path, listing: Result<TrackedTree, NotRun>, request: &GateRequest) -> GateRun {
    let (tree, scope) = match prepare(GATE, Kind::Pin, repo_root, listing, request) {
        Ok(prepared) => prepared,
        Err(stopped) => return stopped,
    };
    let mut run = GateRun::new(
        GATE,
        request.untracked,
        vec![
            format!(
                "==> {GATE}: every folder of {} carries a {README} whose Contents block matches it",
                span_names()
            ),
            scope_line(&scope, &tree),
        ],
    );
    let mut coverage = Tally::default();
    let mut contents = Tally::default();
    let mut violations = 0usize;
    for folder in tree
        .folders()
        .filter(|folder| in_readme_span(folder) && scope.contains(folder))
    {
        let readme = join(folder, README);
        let carries_readme = tree.is_file(&readme);
        let verdict = if carries_readme {
            Verdict::Held
        } else {
            Verdict::failed(format!("{folder}/: no tracked {README}"))
        };
        coverage.count(&verdict);
        run.verdicts.push(verdict);
        if carries_readme {
            let (verdict, found) = judge_readme(repo_root, &tree, folder, &readme);
            contents.count(&verdict);
            violations += found;
            run.verdicts.push(verdict);
        }
    }
    if coverage.judged + contents.judged == 0 {
        run.verdicts
            .push(judged_nothing(GATE, Kind::Pin, repo_root, &scope));
        return run;
    }
    run.totals = vec![
        format!(
            "  coverage: {} folder(s) judged, {} without a tracked {README}",
            coverage.judged, coverage.failed
        ),
        format!(
            "  contents: {} {README} file(s) judged, {} not matching their folder \
             ({violations} violation(s)), {} unreadable",
            contents.judged, contents.failed, contents.unread
        ),
    ];
    run
}

/// The Contents verdict for one tracked README, and how many violations it found.
fn judge_readme(
    repo_root: &Path,
    tree: &TrackedTree,
    folder: &str,
    readme: &str,
) -> (Verdict, usize) {
    let text = match read_tracked(repo_root, readme) {
        Ok(text) => text,
        Err(cause) => {
            let verdict =
                Verdict::did_not_run(format!("{readme} could not be read"), Kind::Pin, cause);
            return (verdict, 0);
        }
    };
    let no_children = FolderChildren::default();
    let children = tree.children(folder).unwrap_or(&no_children);
    let found = contents_violations(&text, folder, children);
    if found.is_empty() {
        return (Verdict::Held, 0);
    }
    let count = found.len();
    let finding = Finding {
        headline: format!("{readme}: Contents does not match the folder ({count} violation(s))"),
        detail: found
            .iter()
            .map(|violation| format!("{readme}:{}: {}", violation.line, violation.message))
            .collect(),
    };
    (Verdict::Failed(finding), count)
}

/// Every way `readme` breaks the Contents grammar or fails to match `children`, in line order.
fn contents_violations(readme: &str, folder: &str, children: &FolderChildren) -> Vec<Violation> {
    let mut violations = match contents_block::read_contents(readme, folder) {
        Err(violation) => vec![violation],
        Ok(block) => {
            let mut found = block.violations;
            found.extend(folder_matching::match_entries(
                children,
                &block.entries,
                block.root_line,
            ));
            found
        }
    };
    violations.sort_by_key(|violation| violation.line);
    violations
}

/// The span's top-level folders, for the header.
fn span_names() -> String {
    let mut names: Vec<&str> = CODE_TREES.to_vec();
    names.push(DOCUMENTATION_ROOT);
    names.join(", ")
}

#[cfg(test)]
#[path = "tests/readme_coverage.rs"]
mod tests;
