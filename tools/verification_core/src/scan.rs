//! Walking a tree and reporting the offending LINES.
//!
//! **Role:** `walk_files` and `matching_lines`: a sorted tree walk and the lines that match.
//!
//! **Position:** called by gates that report every offending line with its path.
//!
//! **Signals and state:** reads the folders and files it walks.
//!
//! **Invariants:** a missing root or an unreadable folder is `DidNotRun`, never zero hits;
//! symbolic links are never followed.
//!
//! [`crate::gate`] answers "does this pattern appear in these files" — one boolean for the whole
//! set. Plenty of gates need the other shape: a recursive search over a directory that prints
//! every hit with its path and line number, so the operator can go and fix them. The file-length
//! and prose gates are that shape.
//!
//! ── FAIL-CLOSED WALKING ──────────────────────────────────────────────────────────────────────
//!
//! The hazard is a search that cannot reach its roots. Silencing the error and treating the
//! absent output as an empty result set turns a renamed directory into *zero violations*, and the
//! gate then prints `clean` forever over a tree it never opened.
//!
//! So [`walk_files`] treats a missing root as [`NotRun::TargetMissing`] and an unreadable
//! directory or file as [`NotRun::Unreadable`]. There is no "skip it quietly" path.
//!
//! No `walkdir` dependency: the recursion is a dozen lines of `std::fs`, and everything links
//! this crate, so its dependency list stays short on purpose.

use std::path::{Path, PathBuf};

use crate::pattern::Pattern;
use crate::verdict::NotRun;

/// One matching line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: PathBuf,
    /// 1-based, as an operator counts lines in an editor.
    pub line_no: usize,
    pub line: String,
}

impl Hit {
    /// `path:line:text` — the shape every consumer of a scan already parses.
    pub fn rendered(&self) -> String {
        format!("{}:{}:{}", self.path.display(), self.line_no, self.line)
    }
}

/// Recursively collect files under `roots`, keeping those `keep` accepts.
///
/// A root that does not exist is a failure, not an empty result — see the module docs.
pub fn walk_files(roots: &[&Path], keep: impl Fn(&Path) -> bool) -> Result<Vec<PathBuf>, NotRun> {
    let mut out = Vec::new();
    for root in roots {
        if !root.exists() {
            return Err(NotRun::TargetMissing(root.to_path_buf()));
        }
        collect(root, &keep, &mut out)?;
    }
    // Deterministic order: a gate's output must not depend on readdir ordering, or two runs over
    // the same tree disagree and no log of either can be compared with the other.
    out.sort();
    Ok(out)
}

fn collect(
    dir: &Path,
    keep: &impl Fn(&Path) -> bool,
    out: &mut Vec<PathBuf>,
) -> Result<(), NotRun> {
    if dir.is_file() {
        if keep(dir) {
            out.push(dir.to_path_buf());
        }
        return Ok(());
    }
    let entries = std::fs::read_dir(dir).map_err(|source| NotRun::Unreadable {
        path: dir.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| NotRun::Unreadable {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let ty = entry.file_type().map_err(|source| NotRun::Unreadable {
            path: path.clone(),
            source,
        })?;
        if ty.is_dir() {
            collect(&path, keep, out)?;
        } else if ty.is_file() && keep(&path) {
            out.push(path);
        }
        // Symlinks are deliberately not followed: a link out of the tree would let a gate report
        // on files that are not in this repository, and a cycle would hang it.
    }
    Ok(())
}

/// Every line in `files` matching `pattern`, in file then line order.
pub fn matching_lines(pattern: &Pattern, files: &[PathBuf]) -> Result<Vec<Hit>, NotRun> {
    let mut hits = Vec::new();
    for path in files {
        let text = match std::fs::read(path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(source) => {
                return Err(NotRun::Unreadable {
                    path: path.clone(),
                    source,
                });
            }
        };
        for (i, line) in text.lines().enumerate() {
            if pattern.is_match(line) {
                hits.push(Hit {
                    path: path.clone(),
                    line_no: i + 1,
                    line: line.to_string(),
                });
            }
        }
    }
    Ok(hits)
}

/// `keep` predicate for a set of file extensions.
pub fn with_extension(exts: &'static [&'static str]) -> impl Fn(&Path) -> bool {
    move |p: &Path| {
        p.extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| exts.contains(&e))
    }
}

#[cfg(test)]
#[path = "tests/scan_tests.rs"]
mod tests;
