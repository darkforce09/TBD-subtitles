//! Every gate, grouped by what it judges: documents, source files and the Cargo workspace.

pub(crate) mod documentation;
pub(crate) mod source;
pub(crate) mod workspace;

use std::path::Path;

use crate::cli::Gate;
use crate::gate_run::GateRequest;
use crate::gate_run::file_rule::verify_file_rule;
use documentation::link_check::{BreakListing, verify_link_check};

/// Run one gate over the repository at `repo_root` and return its exit status: 0 held, 1 a
/// violation, 2 a check that could not run.
pub(crate) fn run(gate: Gate, repo_root: &Path, request: &GateRequest, report: bool) -> u8 {
    match gate {
        Gate::LanguageBans => {
            verify_file_rule(&source::language_bans::LanguageBans, repo_root, request)
        }
        Gate::FileLength => verify_file_rule(&source::file_length::FileLength, repo_root, request),
        Gate::ModuleHeaders => {
            verify_file_rule(&source::module_headers::ModuleHeaders, repo_root, request)
        }
        Gate::TestPlacement => {
            verify_file_rule(&source::test_placement::TestPlacement, repo_root, request)
        }
        Gate::ProseRules => {
            verify_file_rule(&source::prose_rules::ProseRules::new(), repo_root, request)
        }
        Gate::Editorconfig => {
            verify_file_rule(&source::editorconfig::EditorConfig, repo_root, request)
        }
        Gate::CrateLayering => verify_file_rule(
            &workspace::crate_layering::CrateLayering,
            repo_root,
            request,
        ),
        Gate::ReadmeCoverage => {
            documentation::readme_coverage::verify_readme_coverage(repo_root, request)
        }
        Gate::ReadmeSections => verify_file_rule(
            &documentation::readme_sections::ReadmeSections,
            repo_root,
            request,
        ),
        Gate::MarkdownPlacement => {
            documentation::markdown_placement::verify_markdown_placement(repo_root, request)
        }
        Gate::StatusLines => verify_file_rule(
            &documentation::status_lines::StatusLines::new(),
            repo_root,
            request,
        ),
        Gate::LinkCheck => {
            let listing = if report {
                BreakListing::Every
            } else {
                BreakListing::First
            };
            verify_link_check(repo_root, request, listing)
        }
    }
}
