//! Which untracked paths git ignores, asked in one batch.
//!
//! **Role:** answers, for repository-relative paths the tracked tree does not hold, which of them
//! git's ignore rules cover — runtime output such as build trees, export scratch and local
//! secrets — through one `git check-ignore --stdin -z` process per run.
//!
//! **Position:** the backticked-path rule ([`super::backticked_paths`]) hands every path it could
//! not find in the tracked tree to an [`IgnoreRules`] source when the run finishes;
//! [`GitIgnoreRules`] in a real run, a fixed answer in the tests.
//!
//! **Signals and state:** none held; each batch is one child process.
//!
//! **Invariants:** a path counts as ignored only when git names it back; git missing, killed,
//! timed out, exiting with a status other than 0 (some ignored) or 1 (none ignored), or naming a
//! path it was not asked about is a [`NotRun`] cause, never an empty answer that would turn every
//! waiting path into a break.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use verification_core::NotRun;
use verification_core::proc::Run;

/// The program that reads the ignore rules.
const GIT: &str = "git";

/// `git check-ignore --stdin -z`: NUL-separated paths in, the ignored ones back NUL-separated,
/// never quoted.
const CHECK_IGNORE_ARGUMENTS: [&str; 3] = ["check-ignore", "--stdin", "-z"];

/// The exit statuses of a batch that ran: 0 when at least one path is ignored, 1 when none is.
const ANSWERED_STATUSES: [i32; 2] = [0, 1];

/// A batch that takes longer than this is killed: matching paths against the ignore files of a
/// local checkout takes well under a second, so a stall is a broken git.
const CHECK_DEADLINE: Duration = Duration::from_secs(120);

/// Where the backticked-path rule learns which paths git ignores.
pub(super) trait IgnoreRules {
    /// The members of `paths` that git ignores. A path ending in `/` names a folder.
    fn ignored(&self, paths: &[String]) -> Result<BTreeSet<String>, NotRun>;
}

/// The ignore rules of the checkout at a repository root.
pub(super) struct GitIgnoreRules<'a> {
    repo_root: &'a Path,
}

impl<'a> GitIgnoreRules<'a> {
    pub(super) fn new(repo_root: &'a Path) -> GitIgnoreRules<'a> {
        GitIgnoreRules { repo_root }
    }
}

impl IgnoreRules for GitIgnoreRules<'_> {
    fn ignored(&self, paths: &[String]) -> Result<BTreeSet<String>, NotRun> {
        if paths.is_empty() {
            return Ok(BTreeSet::new());
        }
        let request: String = paths.iter().map(|path| format!("{path}\0")).collect();
        let output = Run::new(GIT)
            .args(CHECK_IGNORE_ARGUMENTS)
            .cwd(self.repo_root)
            .stdin(request)
            .timeout(CHECK_DEADLINE)
            .output()?;
        if !ANSWERED_STATUSES.contains(&output.code) {
            return Err(check_problem(output.code, output.stderr.trim()));
        }
        parse_ignored(&output.stdout, paths).map_err(|problem| check_problem(0, &problem))
    }
}

/// The ignored paths in the NUL-separated answer of `git check-ignore -z` to `asked`; a path
/// the batch was not asked about means the answer cannot be trusted.
pub(super) fn parse_ignored(stdout: &str, asked: &[String]) -> Result<BTreeSet<String>, String> {
    stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(|path| {
            if asked.iter().any(|question| question == path) {
                Ok(path.to_string())
            } else {
                Err(format!("git named `{path}`, which it was not asked about"))
            }
        })
        .collect()
}

/// The did-not-run cause for a batch that failed or answered in an unexpected shape.
fn check_problem(status: i32, problem: &str) -> NotRun {
    NotRun::ToolError {
        tool: format!("{GIT} {}", CHECK_IGNORE_ARGUMENTS.join(" ")),
        status,
        stderr: problem.to_string(),
    }
}

#[cfg(test)]
#[path = "tests/git_ignore_rules.rs"]
mod tests;
