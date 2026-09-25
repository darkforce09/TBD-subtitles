//! Test placement: unit tests live in sibling files under `tests/`, never inline.
//!
//! **Role:** the `test-placement` gate. In every production `.rs` file it parses the syntax tree
//! and finds each module enabled by `cfg(test)` and each function marked as a test: an inline
//! test module or a test function fails, and an out-of-line test module must name its file with
//! `#[path = "tests/<file>.rs"]`.
//!
//! **Position:** a [`FileRule`] run by [`crate::gates::run`]; uses `syn` for the syntax tree.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** text inside strings and comments never counts, since only parsed items do;
//! modules nested inside functions and impls are found as well; a file that does not parse fails
//! with the parser's message.

use syn::visit::Visit;
use syn::{Attribute, Expr, ItemFn, ItemMod, Lit, Meta};

use crate::gate_run::file_rule::{FileRule, extension, is_test_path};

/// The folder every test module's file sits in, beside the module that declares it.
const TEST_FOLDER: &str = "tests/";

/// The `test-placement` gate.
pub(crate) struct TestPlacement;

impl FileRule for TestPlacement {
    fn gate(&self) -> &'static str {
        "test-placement"
    }

    fn describes(&self) -> String {
        format!("unit tests live in sibling files under {TEST_FOLDER}, attached with #[path]")
    }

    fn selects(&self, path: &str) -> bool {
        extension(path) == Some("rs") && !is_test_path(path)
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(bytes);
        let file = match syn::parse_file(&text) {
            Ok(file) => file,
            Err(error) => {
                let line = error.span().start().line;
                return vec![format!("{path}:{line}: does not parse: {error}")];
            }
        };
        let mut finder = TestItems {
            path,
            problems: Vec::new(),
        };
        finder.visit_file(&file);
        finder.problems
    }
}

/// Walks a syntax tree and records each misplaced test item.
struct TestItems<'p> {
    path: &'p str,
    problems: Vec<String>,
}

impl<'ast> Visit<'ast> for TestItems<'_> {
    fn visit_item_mod(&mut self, module: &'ast ItemMod) {
        if module.attrs.iter().any(enables_tests) {
            let line = module.ident.span().start().line;
            let name = &module.ident;
            if module.content.is_some() {
                self.problems.push(format!(
                    "{}:{line}: test module `{name}` is inline; move it to a file under \
                     {TEST_FOLDER}",
                    self.path
                ));
            } else if !path_attribute(&module.attrs)
                .is_some_and(|path| path.starts_with(TEST_FOLDER))
            {
                self.problems.push(format!(
                    "{}:{line}: test module `{name}` needs #[path = \"{TEST_FOLDER}<file>.rs\"]",
                    self.path
                ));
            }
        }
        syn::visit::visit_item_mod(self, module);
    }

    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if function.attrs.iter().any(marks_test) {
            let line = function.sig.ident.span().start().line;
            self.problems.push(format!(
                "{}:{line}: test function `{}` sits in a production file",
                self.path, function.sig.ident
            ));
        }
        syn::visit::visit_item_fn(self, function);
    }
}

/// Whether `attribute` is `cfg(...)` naming `test` anywhere inside, such as `cfg(test)` or
/// `cfg(any(test, feature = "x"))`.
fn enables_tests(attribute: &Attribute) -> bool {
    attribute.path().is_ident("cfg")
        && matches!(&attribute.meta, Meta::List(list) if list.tokens.clone().into_iter().any(|token| {
            matches!(&token, proc_macro2::TokenTree::Ident(ident) if ident == "test")
                || matches!(&token, proc_macro2::TokenTree::Group(group)
                    if group.stream().into_iter().any(|inner| {
                        matches!(&inner, proc_macro2::TokenTree::Ident(ident) if ident == "test")
                    }))
        }))
}

/// Whether `attribute` marks a test function: `#[test]` or a path ending in `test`.
fn marks_test(attribute: &Attribute) -> bool {
    attribute
        .path()
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "test")
}

/// The value of a `#[path = "..."]` attribute, if any.
fn path_attribute(attributes: &[Attribute]) -> Option<String> {
    attributes
        .iter()
        .find_map(|attribute| match &attribute.meta {
            Meta::NameValue(pair) if pair.path.is_ident("path") => match &pair.value {
                Expr::Lit(literal) => match &literal.lit {
                    Lit::Str(text) => Some(text.value()),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        })
}

#[cfg(test)]
#[path = "tests/test_placement.rs"]
mod tests;
