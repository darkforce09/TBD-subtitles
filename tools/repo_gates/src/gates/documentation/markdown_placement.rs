//! Markdown placement: documents live in the documentation tree, and live documents stay short.
//!
//! **Role:** the `markdown-placement` gate. Over the tracked files the scope selects it judges
//! two rules: the code trees hold no Markdown file but README.md (test, generated-output and
//! hidden folders excepted); and every live Markdown document under the documentation root is at most [`LIVE_DOCUMENT_LINE_LIMIT`] lines.
//!
//! **Position:** `cargo gates markdown-placement [--path <dir>]... [--with-untracked]`
//! calls [`verify_markdown_placement`]; every region comes from [`super::path_regions`].
//!
//! **Signals and state:** none; one pass over the tracked tree per run.
//!
//! **Invariants:** only the listed files are judged: the tracked ones, and under
//! `--with-untracked` the untracked ones git does not ignore; the frozen records are outside the
//! size limit; a document that cannot be read is "did not
//! run", never a pass.

use std::path::Path;

use verification_core::{Kind, NotRun, Verdict};

use crate::gate_run::gate_scope::GateScope;
use crate::gate_run::path_regions::{
    README, below_exempt_folder, file_name, in_code_tree, in_documentation_root, is_markdown,
    is_size_exempt, parent_folder,
};
use crate::gate_run::tracked_tree::TrackedTree;
use crate::gate_run::{
    GateRequest, GateRun, Tally, judged_nothing, prepare, read_tracked, scope_line,
};
use crate::layout::DOCUMENTATION_ROOT;

/// The gate's name on its header and summary lines.
const GATE: &str = "markdown-placement";

/// The most lines a live document may hold; a longer one splits by topic into a folder with a
/// README index.
const LIVE_DOCUMENT_LINE_LIMIT: usize = 500;

/// `cargo gates markdown-placement`: judge the files `request` lists at `repo_root` over
/// its `--path` scope, print every verdict, and return the exit status (0 held, 1 violation, 2
/// did not run).
pub(crate) fn verify_markdown_placement(repo_root: &Path, request: &GateRequest) -> u8 {
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
    let (tree, scope) = match prepare(GATE, Kind::Ban, repo_root, listing, request) {
        Ok(prepared) => prepared,
        Err(stopped) => return stopped,
    };
    let mut run = GateRun::new(
        GATE,
        request.untracked,
        vec![
            format!(
                "==> {GATE}: code trees hold only {README}, live documents stay at or under {LIVE_DOCUMENT_LINE_LIMIT} lines"
            ),
            scope_line(&scope, &tree),
        ],
    );
    let code_trees = judge_code_trees(&tree, &scope, &mut run.verdicts);
    let documents = judge_documents(repo_root, &tree, &scope, &mut run.verdicts);
    if code_trees.judged + documents.judged == 0 {
        run.verdicts
            .push(judged_nothing(GATE, Kind::Ban, repo_root, &scope));
        return run;
    }
    run.totals = vec![
        format!(
            "  code trees: {} Markdown file(s) judged, {} other than {README}",
            code_trees.judged, code_trees.failed
        ),
        format!(
            "  {DOCUMENTATION_ROOT}/: {} live document(s) judged, {} over \
             {LIVE_DOCUMENT_LINE_LIMIT} lines, {} unreadable",
            documents.judged, documents.failed, documents.unread
        ),
    ];
    run
}

/// Rule 1: every Markdown file in a code tree, outside exempt folders, is a README.md.
fn judge_code_trees(tree: &TrackedTree, scope: &GateScope, verdicts: &mut Vec<Verdict>) -> Tally {
    let mut tally = Tally::default();
    for path in tree.files().filter(|path| {
        scope.contains(path)
            && in_code_tree(path)
            && is_markdown(path)
            && !below_exempt_folder(parent_folder(path))
    }) {
        let verdict = if file_name(path) == README {
            Verdict::Held
        } else {
            Verdict::failed(format!(
                "{path}: Markdown in a code tree; a code tree holds only {README}, and documents \
                 live under {DOCUMENTATION_ROOT}/"
            ))
        };
        tally.count(&verdict);
        verdicts.push(verdict);
    }
    tally
}

/// Rule 2: every live Markdown document under the documentation root is at most
/// [`LIVE_DOCUMENT_LINE_LIMIT`] lines.
fn judge_documents(
    repo_root: &Path,
    tree: &TrackedTree,
    scope: &GateScope,
    verdicts: &mut Vec<Verdict>,
) -> Tally {
    let mut tally = Tally::default();
    for path in tree.files().filter(|path| {
        scope.contains(path)
            && in_documentation_root(path)
            && is_markdown(path)
            && !is_size_exempt(path)
    }) {
        let verdict = match read_tracked(repo_root, path) {
            Err(cause) => {
                Verdict::did_not_run(format!("{path} could not be read"), Kind::Ban, cause)
            }
            Ok(text) => match text.lines().count() {
                lines if lines <= LIVE_DOCUMENT_LINE_LIMIT => Verdict::Held,
                lines => Verdict::failed(format!(
                    "{path}: {lines} lines; a live document stays at or under \
                     {LIVE_DOCUMENT_LINE_LIMIT}, so split it by topic into a folder with a \
                     {README} index"
                )),
            },
        };
        tally.count(&verdict);
        verdicts.push(verdict);
    }
    tally
}

#[cfg(test)]
#[path = "tests/markdown_placement.rs"]
mod tests;
