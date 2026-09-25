//! The files a documentation gate treats as tracked, and the folders they imply.
//!
//! **Role:** runs `git ls-files -z` at the checkout root and, when the operator asks for untracked
//! files (`--with-untracked`), `git ls-files --others --exclude-standard -z` as well; turns the
//! listings into one set of files plus, for every folder that holds one, its direct children.
//!
//! **Position:** the first step of every documentation gate ([`super::prepare`]); the gates and
//! [`super::gate_scope`] read the resulting [`TrackedTree`] and never walk the disk for structure.
//!
//! **Signals and state:** none; a tree is built once per run and read-only afterwards.
//!
//! **Invariants:** a folder exists in the tree exactly when at least one listed file sits at or
//! below it; an untracked file is listed only under [`UntrackedFiles::Included`], and then exactly
//! like a tracked one; a file git ignores is never listed; either listing missing, killed, timed
//! out or exiting non-zero is a [`NotRun`] cause, never an empty tree that would read as a clean
//! pass.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::time::Duration;

use verification_core::NotRun;
use verification_core::proc::{Output, Run};

/// The program that lists the files.
const GIT: &str = "git";

/// `git ls-files -z`: every path in the index, NUL-separated and never quoted.
const TRACKED_LISTING: [&str; 2] = ["ls-files", "-z"];

/// `git ls-files --others --exclude-standard -z`: every file outside the index that git's
/// standard ignore rules (`.gitignore` files, `.git/info/exclude`, `core.excludesFile`) leave
/// alone, NUL-separated and never quoted.
const UNTRACKED_LISTING: [&str; 4] = ["ls-files", "--others", "--exclude-standard", "-z"];

/// A listing that takes longer than this is killed and reported as a timeout: either listing of
/// this repository finishes in well under a second, so a stall is a broken git, not a large tree.
const LISTING_DEADLINE: Duration = Duration::from_secs(120);

/// Whether a gate sees the untracked files git does not ignore.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum UntrackedFiles {
    /// The index alone, the committed view CI judges: an untracked file is invisible.
    #[default]
    Invisible,
    /// The index plus every untracked file git does not ignore, each judged exactly like a
    /// tracked one (`--with-untracked`), so new files can be checked before they are committed.
    Included,
}

/// The listed children directly inside one folder.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct FolderChildren {
    /// Names of the listed files directly inside the folder.
    pub(crate) files: BTreeSet<String>,
    /// Names of the folders directly inside the folder that hold a listed file.
    pub(crate) folders: BTreeSet<String>,
}

/// The files a gate treats as tracked and every folder that holds one.
#[derive(Debug, Default)]
pub(crate) struct TrackedTree {
    /// Every listed file, as a `/`-separated repository-relative path.
    files: BTreeSet<String>,
    /// Every folder that holds a listed file, keyed by its repository-relative path; the
    /// repository root is the empty path.
    folders: BTreeMap<String, FolderChildren>,
    /// How many of `files` came from the untracked listing; `None` when the tree was listed
    /// without it.
    untracked_files: Option<usize>,
}

impl TrackedTree {
    /// List the files at `repo_root`: the index with `git ls-files -z`, joined under
    /// [`UntrackedFiles::Included`] by the untracked files git does not ignore.
    pub(crate) fn load(repo_root: &Path, untracked: UntrackedFiles) -> Result<TrackedTree, NotRun> {
        let mut tree = TrackedTree::from_listing(&run_listing(GIT, repo_root, &TRACKED_LISTING)?);
        if untracked == UntrackedFiles::Included {
            tree.add_untracked(&run_listing(GIT, repo_root, &UNTRACKED_LISTING)?);
        }
        Ok(tree)
    }

    /// Build the tree from a NUL-separated listing of tracked files.
    pub(crate) fn from_listing(listing: &str) -> TrackedTree {
        let mut tree = TrackedTree::default();
        for path in listed_paths(listing) {
            tree.insert(path);
        }
        tree
    }

    /// Add the files of a NUL-separated untracked listing, counting the ones the tree did not
    /// already hold.
    pub(crate) fn add_untracked(&mut self, listing: &str) {
        let mut added = 0;
        for path in listed_paths(listing) {
            if self.insert(path) {
                added += 1;
            }
        }
        self.untracked_files = Some(self.untracked_files.unwrap_or(0) + added);
    }

    /// Record one file and every folder above it; `false` when the tree already held it.
    fn insert(&mut self, path: &str) -> bool {
        if !self.files.insert(path.to_string()) {
            return false;
        }
        let (mut folder, name) = split_parent(path);
        self.folders
            .entry(folder.to_string())
            .or_default()
            .files
            .insert(name.to_string());
        while !folder.is_empty() {
            let (parent, name) = split_parent(folder);
            let children = self.folders.entry(parent.to_string()).or_default();
            if !children.folders.insert(name.to_string()) {
                break;
            }
            folder = parent;
        }
        true
    }

    /// Every listed file, in path order.
    pub(crate) fn files(&self) -> impl Iterator<Item = &str> {
        self.files.iter().map(String::as_str)
    }

    /// Every folder that holds a listed file, in path order, without the repository root.
    pub(crate) fn folders(&self) -> impl Iterator<Item = &str> {
        self.folders
            .keys()
            .map(String::as_str)
            .filter(|folder| !folder.is_empty())
    }

    /// How many listed files are tracked.
    pub(crate) fn tracked_file_count(&self) -> usize {
        self.files.len() - self.untracked_files.unwrap_or(0)
    }

    /// How many listed files are untracked; `None` when the tree was listed without them.
    pub(crate) fn untracked_file_count(&self) -> Option<usize> {
        self.untracked_files
    }

    /// Whether `path` is a listed file.
    pub(crate) fn is_file(&self, path: &str) -> bool {
        self.files.contains(path)
    }

    /// Whether `path` is a folder that holds a listed file; the empty path is the repository
    /// root, which holds one whenever anything is listed.
    pub(crate) fn is_folder(&self, path: &str) -> bool {
        self.folders.contains_key(path)
    }

    /// The listed children directly inside `folder`, or `None` when it holds no listed file.
    pub(crate) fn children(&self, folder: &str) -> Option<&FolderChildren> {
        self.folders.get(folder)
    }
}

/// Run `program` with a listing's `arguments` at `repo_root`, and yield what it printed or the
/// reason it printed nothing usable.
fn run_listing(program: &str, repo_root: &Path, arguments: &[&str]) -> Result<String, NotRun> {
    let output = Run::new(program)
        .args(arguments)
        .cwd(repo_root)
        .timeout(LISTING_DEADLINE)
        .output()?;
    listing_text(output, program, arguments)
}

/// A finished listing's text, or [`NotRun::ToolError`] naming the listing when it exited
/// non-zero: a failed listing lists nothing, and nothing must never read as a clean tree.
fn listing_text(output: Output, program: &str, arguments: &[&str]) -> Result<String, NotRun> {
    if output.code != 0 {
        return Err(NotRun::ToolError {
            tool: format!("{program} {}", arguments.join(" ")),
            status: output.code,
            stderr: output.stderr.trim().to_string(),
        });
    }
    Ok(output.stdout)
}

/// The file paths of a NUL-separated listing. Empty entries fall away, and so does an entry
/// ending in `/`: the untracked listing names a nested repository that way without descending
/// into it, and it holds no file this checkout lists.
fn listed_paths(listing: &str) -> impl Iterator<Item = &str> {
    listing
        .split('\0')
        .filter(|path| !path.is_empty() && !path.ends_with('/'))
}

/// `(folder, name)` for a repository-relative path; a top-level name sits in the root, `""`.
fn split_parent(path: &str) -> (&str, &str) {
    path.rsplit_once('/').unwrap_or(("", path))
}

#[cfg(test)]
#[path = "tests/tracked_tree.rs"]
mod tests;
