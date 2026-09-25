//! Crate layering: a crate depends only on crates of a lower layer.
//!
//! **Role:** the `crate-layering` gate. Every `Cargo.toml` in the code trees names a package the
//! layer table in [`crate::layout`] knows; a product crate may depend only on product crates of a
//! strictly lower layer, and a tool only on the crates the table lists for it. Every dependency
//! table counts: normal, dev and build, target-specific ones included, and a `package =` rename
//! is followed to the real name.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`]; parses manifests with `toml`.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a crate missing from the layer table fails rather than passing unjudged; a
//! manifest that does not parse fails with the parser's message.

use toml::Value;

use crate::gate_run::file_rule::FileRule;
use crate::gate_run::path_regions::{file_name, in_code_tree};
use crate::layout::{PRODUCT_LAYERS, TOOL_DEPENDENCIES};

/// The dependency tables of a manifest or of one of its `[target.*]` sections.
const DEPENDENCY_TABLES: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// The `crate-layering` gate.
pub(crate) struct CrateLayering;

impl FileRule for CrateLayering {
    fn gate(&self) -> &'static str {
        "crate-layering"
    }

    fn describes(&self) -> String {
        "a crate depends only on crates of a lower layer; tools only on the crates listed for them"
            .to_string()
    }

    fn selects(&self, path: &str) -> bool {
        in_code_tree(path) && file_name(path) == "Cargo.toml"
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let manifest: Value = match toml::from_str(&String::from_utf8_lossy(bytes)) {
            Ok(manifest) => manifest,
            Err(error) => return vec![format!("{path}: does not parse: {error}")],
        };
        let Some(name) = manifest
            .get("package")
            .and_then(|package| package.get("name"))
            .and_then(Value::as_str)
        else {
            return vec![format!("{path}: no [package] name")];
        };
        let allowed: Box<dyn Fn(&str) -> bool> = if let Some(layer) = product_layer(name) {
            Box::new(move |dependency| product_layer(dependency).is_some_and(|below| below < layer))
        } else if let Some((_, allowed)) = TOOL_DEPENDENCIES.iter().find(|(tool, _)| *tool == name)
        {
            Box::new(move |dependency| allowed.contains(&dependency))
        } else {
            return vec![format!(
                "{path}: crate `{name}` is in no layer; add it to the layer table in \
                 tools/repo_gates/src/layout.rs"
            )];
        };
        workspace_dependencies(&manifest)
            .into_iter()
            .filter(|dependency| !allowed(dependency))
            .map(|dependency| {
                format!("{path}: `{name}` may not depend on `{dependency}`, which is not below it")
            })
            .collect()
    }
}

/// The layer of a product crate, or `None` when `name` is no product crate.
fn product_layer(name: &str) -> Option<u8> {
    PRODUCT_LAYERS
        .iter()
        .find(|(product, _)| *product == name)
        .map(|(_, layer)| *layer)
}

/// Whether `name` is a crate of this workspace: a product crate or a tool.
fn is_workspace_crate(name: &str) -> bool {
    product_layer(name).is_some() || TOOL_DEPENDENCIES.iter().any(|(tool, _)| *tool == name)
}

/// Every workspace crate the manifest depends on, from every dependency table, renames followed.
fn workspace_dependencies(manifest: &Value) -> Vec<String> {
    let mut sections = vec![manifest];
    if let Some(targets) = manifest.get("target").and_then(Value::as_table) {
        sections.extend(targets.values());
    }
    let mut found = Vec::new();
    for section in sections {
        for table in DEPENDENCY_TABLES {
            let Some(entries) = section.get(table).and_then(Value::as_table) else {
                continue;
            };
            for (key, entry) in entries {
                let real = entry.get("package").and_then(Value::as_str).unwrap_or(key);
                if is_workspace_crate(real) && !found.iter().any(|seen| seen == real) {
                    found.push(real.to_string());
                }
            }
        }
    }
    found
}

#[cfg(test)]
#[path = "tests/crate_layering.rs"]
mod tests;
