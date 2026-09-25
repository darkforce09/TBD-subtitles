//! The `--path` scope a documentation gate judges.
//!
//! **Role:** turns the operator's repeatable `--path <dir>` values into repository-relative
//! folders, refuses a value that cannot name a tracked folder, and answers whether a path lies in
//! the scope.
//!
//! **Position:** fed by the verify command line through [`super::prepare`]; checks each folder
//! against the [`super::tracked_tree::TrackedTree`] the gate already listed.
//!
//! **Signals and state:** none; a resolved scope is immutable.
//!
//! **Invariants:** no value, `.`, or the checkout root itself means the whole repository; a value
//! that climbs out with `..`, lies outside the checkout, names a file or names no tracked folder
//! is refused, so a typo can never narrow a gate to nothing and read as a pass.

use std::path::{Component, Path, PathBuf};

use super::path_regions::is_within;
use super::tracked_tree::TrackedTree;

/// The folders a gate judges; empty means the whole repository.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct GateScope {
    folders: Vec<String>,
}

/// A `--path` value the scope refuses, with the reason.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ScopeRefusal {
    /// The value as the operator wrote it.
    pub(crate) value: String,
    /// Why it names no tracked folder, phrased to follow the value.
    pub(crate) reason: &'static str,
}

impl GateScope {
    /// Resolve every `--path` value against the checkout root and the tracked tree.
    ///
    /// Every value is checked, even when another one already selects the whole repository, so a
    /// typo is refused wherever it sits in the command line.
    pub(crate) fn resolve(
        values: &[String],
        repo_root: &Path,
        tree: &TrackedTree,
    ) -> Result<GateScope, ScopeRefusal> {
        let mut folders: Vec<String> = Vec::new();
        let mut whole_repository = values.is_empty();
        for value in values {
            let refuse = |reason| ScopeRefusal {
                value: value.clone(),
                reason,
            };
            let folder = normalise(value, repo_root).map_err(refuse)?;
            if folder.is_empty() {
                whole_repository = true;
            } else if tree.is_file(&folder) {
                return Err(refuse("is a file; --path takes a folder"));
            } else if !tree.is_folder(&folder) {
                return Err(refuse("names no tracked folder"));
            } else if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        if whole_repository {
            folders.clear();
        }
        Ok(GateScope { folders })
    }

    /// Whether `path` lies in the scope.
    pub(crate) fn contains(&self, path: &str) -> bool {
        self.folders.is_empty() || self.folders.iter().any(|folder| is_within(path, folder))
    }

    /// The scope as the gate's header and refusals name it.
    pub(crate) fn describe(&self) -> String {
        if self.folders.is_empty() {
            "the whole repository".to_string()
        } else {
            self.folders.join(", ")
        }
    }

    /// The path a "judged nothing" refusal points at: the first scope folder, or the checkout
    /// root for the whole repository.
    pub(crate) fn anchor(&self, repo_root: &Path) -> PathBuf {
        self.folders
            .first()
            .map_or_else(|| repo_root.to_path_buf(), |folder| repo_root.join(folder))
    }
}

/// A `--path` value as a `/`-separated repository-relative folder; the empty string is the whole
/// repository. `./`, repeated and trailing separators fall away; an absolute value must lie
/// inside the checkout.
fn normalise(value: &str, repo_root: &Path) -> Result<String, &'static str> {
    let path = Path::new(value.trim());
    let relative = if path.is_absolute() {
        path.strip_prefix(repo_root)
            .map_err(|_| "lies outside the checkout")?
    } else {
        path
    };
    let mut parts: Vec<&str> = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str().ok_or("is not UTF-8")?),
            Component::CurDir => {}
            Component::ParentDir => return Err("climbs out of its folder with `..`"),
            Component::RootDir | Component::Prefix(_) => return Err("lies outside the checkout"),
        }
    }
    Ok(parts.join("/"))
}

#[cfg(test)]
#[path = "tests/gate_scope.rs"]
mod tests;
