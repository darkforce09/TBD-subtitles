//! The command line: which gate to run, over which part of the repository.
//!
//! **Role:** declares `cargo gates [<gate>] [--report] [--path <dir>]... [--with-untracked]` with
//! clap and turns the arguments into the [`GateRequest`] every gate judges.
//!
//! **Position:** parsed by `main`; the link check walks the same command tree to judge every
//! `cargo gates` command a document cites.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** the gate names are the kebab-case [`Gate`] variants, in run order; no gate named
//! means every gate.

use clap::{Args, Parser, ValueEnum};

use crate::gate_run::{GateRequest, UntrackedFiles};

/// Checks the repository laws a program can check. With no gate named, runs every gate.
#[derive(Debug, Parser)]
#[command(name = "cargo gates", bin_name = "cargo gates", version, about)]
pub(crate) struct Cli {
    /// The gate to run; every gate when left out.
    #[arg(value_enum)]
    pub(crate) gate: Option<Gate>,
    /// Print every link-check break in full instead of the first ones.
    #[arg(long)]
    pub(crate) report: bool,
    #[command(flatten)]
    pub(crate) arguments: GateArgs,
}

/// The arguments every gate takes.
#[derive(Debug, Args)]
pub(crate) struct GateArgs {
    /// Judge only what lies at or under this repository-relative folder (repeatable).
    #[arg(long = "path", value_name = "DIR")]
    pub(crate) paths: Vec<String>,
    /// Also judge the untracked files git does not ignore, exactly like tracked ones: a check of
    /// new files before they are committed.
    #[arg(long = "with-untracked")]
    pub(crate) with_untracked: bool,
}

impl GateArgs {
    /// The request every gate judges.
    pub(crate) fn request(&self) -> GateRequest {
        GateRequest {
            paths: self.paths.clone(),
            untracked: if self.with_untracked {
                UntrackedFiles::Included
            } else {
                UntrackedFiles::Invisible
            },
        }
    }
}

/// One gate. The order is the order `cargo gates` runs them in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum Gate {
    /// Every tracked file is Rust, Markdown, TOML or JSON; no shell, Python, Node or Makefile.
    LanguageBans,
    /// Production Rust files stay under 500 lines, test files under 1000.
    FileLength,
    /// Crate roots and long files open with the Role, Position, Signals and state, Invariants header.
    ModuleHeaders,
    /// Unit tests live in sibling files under tests/, never inline.
    TestPlacement,
    /// Code and documents carry no ticket or milestone ids and no history words.
    ProseRules,
    /// UTF-8, LF line ends, a final newline, no trailing whitespace outside Markdown.
    Editorconfig,
    /// A crate depends only on crates of a lower layer.
    CrateLayering,
    /// Every folder in the code trees and the documentation root has a README.md whose Contents
    /// block lists the folder exactly.
    ReadmeCoverage,
    /// Every README.md holds the core sections in order, and a code README's Boundaries has
    /// exactly the three bullets.
    ReadmeSections,
    /// Code trees hold no Markdown but README.md; live documents stay at or under 500 lines.
    MarkdownPlacement,
    /// Every document under the documentation root opens with a valid status line.
    StatusLines,
    /// Every link, backticked repository path and cited `cargo gates` command exists.
    LinkCheck,
}

impl Gate {
    /// Every gate, in run order.
    pub(crate) fn all() -> &'static [Gate] {
        Gate::value_variants()
    }
}
