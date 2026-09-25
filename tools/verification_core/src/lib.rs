//! The fail-closed check library every repository gate is written with.
//!
//! **Role:** one implementation of what a check can conclude — held, failed, or did not run —
//! shared by every gate, so no gate can report a pass over an input it never examined.
//!
//! **Position:** used by `tools/repo_gates`; depends on `regex` and `crates/child_process`.
//! [`verdict`] holds the outcome type and its rendering; [`gate`] the assertions written against
//! it; [`pattern`] the compiled search patterns; [`scan`] the fail-closed tree walk; [`report`]
//! the accumulation and the exit contract; [`proc`] child processes whose every stop reason stays
//! a verdict.
//!
//! **Signals and state:** reads the files a check names; runs the programs a check names;
//! prints failures to stdout as they land.
//!
//! **Invariants:**
//!
//! 1. "The check did not run" never folds into "the check passed". [`Verdict`] has no `bool`
//!    conversion of any kind, and adding a [`NotRun`] variant breaks every incomplete `match`.
//! 2. No search tool can go absent: the matcher is the `regex` crate, compiled in.
//! 3. Compound conditions cannot short-circuit clean: [`gate::probe_files`] returns
//!    `Result<bool, NotRun>`, so `?` carries "did not run" upward.
//! 4. Failures render as one headline plus six-space continuation lines in every gate, so one
//!    log reads like another.
//!
//! ```no_run
//! use std::path::Path;
//! use verification_core::{gate, Pattern, Report};
//!
//! let mut report = Report::new("verify-example");
//! let src = [Path::new("src/lib.rs")];
//!
//! report.check(gate::ban(
//!     "no stray dbg! in committed code",
//!     &Pattern::literal("dbg!("),
//!     &src,
//! ));
//!
//! std::process::exit(report.finish());
//! ```

pub mod gate;
pub mod pattern;
pub mod proc;
pub mod report;
pub mod scan;
pub mod verdict;

pub use pattern::Pattern;
pub use report::Report;
pub use verdict::{Finding, Kind, NotRun, Verdict};
