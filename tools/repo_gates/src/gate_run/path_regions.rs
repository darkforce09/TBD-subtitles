//! Where a tracked path sits, for the documentation gates.
//!
//! **Role:** answers the region questions the gates ask of a repository-relative path: is it
//! inside a folder, inside a code tree, inside the README span, below a folder exempt by its name,
//! a Markdown file, or inside an area the size limit leaves alone.
//!
//! **Position:** reads [`crate::layout`]; called by
//! [`super::readme_coverage`], [`super::markdown_placement`] and [`super::gate_scope`].
//!
//! **Signals and state:** none; pure functions over `/`-separated repository-relative paths.
//!
//! **Invariants:** a path is inside a folder only when it equals the folder or continues it after
//! a `/`, so `apps_extra/x` is never inside `apps`; the empty folder is the repository root and
//! holds every path.

use crate::layout::{CODE_TREES, DOCUMENTATION_ROOT, FROZEN_FOLDERS};

/// The file every folder in the README span carries, and the only Markdown file name a code tree
/// may hold.
pub(crate) const README: &str = "README.md";

/// Folder names that exempt a folder, with everything below it, from the README rules and the
/// code-tree Markdown rule: test sources and generated output. A folder whose name starts with `.`
/// (hidden tool configuration) is exempt the same way.
const EXEMPT_FOLDER_NAMES: [&str; 2] = ["tests", "generated"];

/// Whether `path` is `folder` itself or lies below it. The empty folder is the repository root.
pub(crate) fn is_within(path: &str, folder: &str) -> bool {
    folder.is_empty()
        || path
            .strip_prefix(folder)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
}

/// The folder holding `path`; a top-level path sits in the repository root, `""`.
pub(crate) fn parent_folder(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// The last component of `path`.
pub(crate) fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// `folder/name`, or `name` alone in the repository root.
pub(crate) fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_string()
    } else {
        format!("{folder}/{name}")
    }
}

/// Whether `path` lies in one of the code trees.
pub(crate) fn in_code_tree(path: &str) -> bool {
    CODE_TREES.iter().any(|tree| is_within(path, tree))
}

/// Whether `path` lies under the documentation root.
pub(crate) fn in_documentation_root(path: &str) -> bool {
    is_within(path, DOCUMENTATION_ROOT)
}

/// Whether `folder` belongs to the README span, the one set of folders both README rules judge: a
/// code tree or the documentation root, the roots themselves included, minus the folders exempt
/// by their name ([`below_exempt_folder`]) with everything below them. A README.md inside an exempt folder is neither required nor checked.
pub(crate) fn in_readme_span(folder: &str) -> bool {
    (in_code_tree(folder) || in_documentation_root(folder)) && !below_exempt_folder(folder)
}

/// Whether any component of `folder` is a test folder, a generated-output folder or a hidden
/// folder, which exempts it with everything below from the README rules and the code-tree
/// Markdown rule.
pub(crate) fn below_exempt_folder(folder: &str) -> bool {
    folder
        .split('/')
        .any(|name| EXEMPT_FOLDER_NAMES.contains(&name) || name.starts_with('.'))
}

/// Whether `path` names a Markdown file: its extension is `md` in any letter case.
pub(crate) fn is_markdown(path: &str) -> bool {
    file_name(path)
        .rsplit_once('.')
        .is_some_and(|(stem, extension)| !stem.is_empty() && extension.eq_ignore_ascii_case("md"))
}

/// Whether a document under the documentation root is outside the size limit: the frozen
/// records.
pub(crate) fn is_size_exempt(path: &str) -> bool {
    is_frozen_record(path)
}

/// Whether `path` is a frozen record: a document other than a README.md inside a frozen folder.
pub(crate) fn is_frozen_record(path: &str) -> bool {
    file_name(path) != README && FROZEN_FOLDERS.iter().any(|folder| is_within(path, folder))
}

#[cfg(test)]
#[path = "tests/path_regions.rs"]
mod tests;
