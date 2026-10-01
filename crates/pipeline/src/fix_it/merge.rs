//! Fix It's kept changes put into the corrections, the owner's own corrections first.
//!
//! **Role:** write each kept or accepted line as a Fix It correction with the model and the
//! reason, and leave every line the owner settled, even one settled while Fix It ran.
//!
//! **Position:** called by `fix_it::fix_job` inside `work_dir::update_corrections`, so it sees
//! the stored corrections as that write transaction reads them.
//!
//! **Signals and state:** changes the corrections and marks each line it applied.
//!
//! **Invariants:** an owner's correction is never replaced; an earlier Fix It change the owner
//! has not checked may be; a line whose verdict writes no correction is left as it is.

use job_model::outputs::{Chosen, Correction, Corrections, LineFix};

/// Which lines went in, and which kept the owner's correction instead.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Merged {
    pub applied: Vec<String>,
    pub kept_yours: Vec<String>,
}

/// Put `lines`' corrections into `corrections` as `model`'s Fix It changes.
pub fn merge(corrections: &mut Corrections, lines: &mut [LineFix], model: &str) -> Merged {
    let mut merged = Merged::default();
    for line in lines.iter_mut() {
        line.applied = false;
        if !line.verdict.writes_correction() {
            continue;
        }
        if corrections.by_owner(&line.id) {
            merged.kept_yours.push(line.id.clone());
            continue;
        }
        corrections.set(Correction {
            id: line.id.clone(),
            text: line.after_text.clone(),
            flags: line.after_flags.clone(),
            chosen: Chosen::FixIt {
                model: model.to_string(),
                why: line.why(),
            },
        });
        line.applied = true;
        merged.applied.push(line.id.clone());
    }
    merged
}

#[cfg(test)]
#[path = "tests/merge.rs"]
mod tests;
