//! A throwaway checkout on disk for the documentation gate tests.
//!
//! Tracked files are written and listed; untracked files are only written; a listed-only path is
//! in the listing but absent from the disk. The gates judge the listing, never the disk walk, so
//! the three kinds cover every way the two can disagree. [`FixtureCheckout::tree`] builds the
//! listing directly; [`FixtureCheckout::listed_by_git`] makes the checkout a git repository and
//! runs the real listing, so git's own ignore rules decide which untracked files it sees.

use std::path::{Path, PathBuf};

use verification_core::proc::Run;
use verification_core::{NotRun, Verdict};

use super::tracked_tree::TrackedTree;
use super::{GateRequest, GateRun, UntrackedFiles};

/// Variables that would point git at another repository or index than the fixture's own.
const GIT_LOCATION_VARIABLES: [&str; 3] = ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE"];

/// A temporary checkout root plus the listing a `git ls-files` there would print.
pub(crate) struct FixtureCheckout {
    root: PathBuf,
    listing: Vec<String>,
}

impl FixtureCheckout {
    /// An empty checkout under the system temporary folder, unique per test tag and process.
    pub(crate) fn new(tag: &str) -> FixtureCheckout {
        let root =
            std::env::temp_dir().join(format!("documentation-gates-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create the fixture checkout");
        FixtureCheckout {
            root,
            listing: Vec::new(),
        }
    }

    /// Write `path` and list it as tracked.
    pub(crate) fn tracked(&mut self, path: &str, text: &str) -> &mut FixtureCheckout {
        self.write(path, text);
        self.listing.push(path.to_string());
        self
    }

    /// Write `path` without listing it.
    pub(crate) fn untracked(&mut self, path: &str, text: &str) -> &mut FixtureCheckout {
        self.write(path, text);
        self
    }

    /// List `path` as tracked without writing it.
    pub(crate) fn listed_only(&mut self, path: &str) -> &mut FixtureCheckout {
        self.listing.push(path.to_string());
        self
    }

    /// The tree the listing describes.
    pub(crate) fn tree(&self) -> TrackedTree {
        TrackedTree::from_listing(&self.listing.join("\0"))
    }

    /// The tree a gate run lists: the checkout becomes a git repository whose index holds the
    /// tracked paths, then the real listing runs. Every tracked path must exist on disk, since
    /// `git add` records what the disk holds.
    pub(crate) fn listed_by_git(&self, untracked: UntrackedFiles) -> Result<TrackedTree, NotRun> {
        self.git(&["init", "--quiet"]);
        let mut add = vec!["add", "--"];
        add.extend(self.listing.iter().map(String::as_str));
        self.git(&add);
        TrackedTree::load(&self.root, untracked)
    }

    /// The checkout root.
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    fn write(&self, path: &str, text: &str) {
        let full = self.root.join(path);
        if let Some(folder) = full.parent() {
            std::fs::create_dir_all(folder).expect("create a fixture folder");
        }
        std::fs::write(&full, text).expect("write a fixture file");
    }

    /// Run git in the checkout, never in the repository or index the environment names.
    fn git(&self, arguments: &[&str]) {
        let command = GIT_LOCATION_VARIABLES.iter().fold(
            Run::new("git").args(arguments).cwd(&self.root),
            |run, variable| run.env_remove(*variable),
        );
        let output = command.output().expect("git runs in the fixture checkout");
        assert_eq!(
            output.code, 0,
            "git {arguments:?} failed: {}",
            output.stderr
        );
    }
}

impl Drop for FixtureCheckout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A request over the `--path` values `scope` that sees untracked files as `untracked` says.
pub(crate) fn request(scope: &[&str], untracked: UntrackedFiles) -> GateRequest {
    GateRequest {
        paths: scope.iter().map(ToString::to_string).collect(),
        untracked,
    }
}

/// A README whose Contents block is `root` followed by `entries`, between an H1 and a closing
/// section, the shape the README standard asks for.
pub(crate) fn readme_with_contents(root: &str, entries: &[&str]) -> String {
    let mut block = vec![root];
    block.extend_from_slice(entries);
    format!(
        "# Fixture\n\nWhat the folder holds.\n\n## Contents\n\n```text\n{}\n```\n\n## Boundaries\n\n\
         - Depends on: nothing.\n",
        block.join("\n")
    )
}

/// The rendered text of every verdict of `run` that did not hold, in order.
pub(crate) fn failures(run: &GateRun) -> Vec<String> {
    run.verdicts
        .iter()
        .filter(|verdict| !matches!(verdict, Verdict::Held))
        .map(ToString::to_string)
        .collect()
}

/// How many verdicts of `run` held, broke a rule, and did not run.
pub(crate) fn outcome_counts(run: &GateRun) -> (usize, usize, usize) {
    run.verdicts.iter().fold(
        (0, 0, 0),
        |(held, failed, not_run), verdict| match verdict {
            Verdict::Held => (held + 1, failed, not_run),
            Verdict::Failed(_) => (held, failed + 1, not_run),
            Verdict::DidNotRun(..) => (held, failed, not_run + 1),
        },
    )
}
