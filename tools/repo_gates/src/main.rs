//! The repository gate runner: `cargo gates [<gate>] [--path <dir>]... [--with-untracked]`.
//!
//! **Role:** runs the checks of the repository laws a program can check — languages, file
//! lengths, module headers, test placement, prose, whitespace, crate layering, READMEs, document
//! placement, status lines and links — over the tracked files, and exits 0 when every check held,
//! 1 when one found a violation, 2 when one could not run.
//!
//! **Position:** a tool, run by people and agents before a commit; `cli` parses, each module under
//! `gates` judges one law with the shared machinery in `gate_run`, and `layout` names the
//! repository's places. Depends on `verification_core`.
//!
//! **Signals and state:** reads the tracked files through `git ls-files`; prints to stdout.
//!
//! **Invariants:** a gate that could not list or read what it judges reports "did not run",
//! never a pass; running every gate exits with the worst gate's status.

mod cli;
mod gate_run;
mod gates;
mod layout;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

use cli::{Cli, Gate};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(repo_root) = repository_root() else {
        eprintln!("cargo gates: no Cargo workspace root found above the working directory");
        return ExitCode::from(2);
    };
    let request = cli.arguments.request();
    let gates: Vec<Gate> = match cli.gate {
        Some(gate) => vec![gate],
        None => Gate::all().to_vec(),
    };
    let mut worst = 0;
    for gate in gates {
        let status = gates::run(gate, &repo_root, &request, cli.report);
        println!();
        worst = worst.max(status);
    }
    ExitCode::from(worst)
}

/// The workspace root: the nearest folder at or above the working directory whose `Cargo.toml`
/// declares `[workspace]`.
fn repository_root() -> Option<PathBuf> {
    let start = std::env::current_dir().ok()?;
    start
        .ancestors()
        .find(|folder| is_workspace_root(folder))
        .map(Path::to_path_buf)
}

fn is_workspace_root(folder: &Path) -> bool {
    std::fs::read_to_string(folder.join("Cargo.toml"))
        .is_ok_and(|manifest| manifest.lines().any(|line| line.trim() == "[workspace]"))
}

/// The checkout this crate sits in, for tests that judge the real repository.
#[cfg(test)]
fn test_repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the checkout root exists")
}
