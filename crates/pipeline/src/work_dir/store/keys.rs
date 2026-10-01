//! Keys of `job.redb`: the one place that names the row of every document a step, the runner or
//! the owner keeps.
//!
//! **Role:** name a step's output documents (`outputs/<step>`, `outputs/<step>/<part>` for a
//! step's further documents), its record (`step_records/<step>`), the job record, the table
//! layout, the owner's corrections and Fix It's record.
//!
//! **Position:** used by `kinds` to type each key, by `graph` for what a step reads, by
//! `tasks::io` to address what a task reads and writes, and by `resume` and the runner.
//!
//! **Signals and state:** none; constants and pure functions.
//!
//! **Invariants:** every document a step writes is listed in [`output_parts`], so a step's rows
//! are exactly [`output_keys`]; a part name never contains `/`.

use job_model::StepName;
use worker_channel::address::{Address, Key, Table};

/// The `meta` row of the job record: the video, its identity and the settings.
pub const JOB_RECORD: &str = "job_record";

/// The `meta` row of every table's layout version.
pub const LAYOUT: &str = super::tables::LAYOUT_KEY;

/// The `corrections` row of the owner's line corrections.
pub const LINE_CORRECTIONS: &str = "lines";

/// The `corrections` row of the owner's on-screen text corrections.
pub const TEXT_CORRECTIONS: &str = "text";

/// The `corrections` row of Fix It's record of its runs. It lives beside the owner's corrections
/// because the owner keeps or undoes each change it holds, and no step rerun clears it.
pub const FIX_RECORD: &str = "fix";

/// The cues step's second document: the sound cues that found no place.
pub const DROPPED_SOUNDS: &str = "dropped_sounds";

/// The typesetting step's second document: the ASS events of the combined subtitle file.
pub const TYPESET_ASS: &str = "ass";

/// The documents `step` writes: `None` for its main document, a part name for each further one.
/// A step that keeps only files (the separation's stems) writes none.
pub fn output_parts(step: StepName) -> &'static [Option<&'static str>] {
    match step {
        StepName::Separation => &[],
        StepName::Cues => &[None, Some(DROPPED_SOUNDS)],
        StepName::TextTypeset => &[None, Some(TYPESET_ASS)],
        _ => &[None],
    }
}

/// The `outputs` name of `step`'s document `part`: `<step>`, or `<step>/<part>`.
pub fn output_name(step: StepName, part: Option<&str>) -> String {
    match part {
        None => step.as_str().to_string(),
        Some(part) => format!("{}/{part}", step.as_str()),
    }
}

/// The `outputs` key of `step`'s document `part`.
pub fn output_key(step: StepName, part: Option<&str>) -> Key {
    Key::Name(output_name(step, part))
}

/// The address of `step`'s document `part`.
pub fn output_address(step: StepName, part: Option<&str>) -> Address {
    Address {
        table: Table::Outputs,
        key: output_key(step, part),
    }
}

/// Every `outputs` key `step` writes.
pub fn output_keys(step: StepName) -> Vec<Key> {
    output_parts(step)
        .iter()
        .map(|part| output_key(step, *part))
        .collect()
}

/// The `step_records` key of `step`.
pub fn record_key(step: StepName) -> Key {
    Key::Name(step.as_str().to_string())
}

/// A named key.
pub fn named(name: &str) -> Key {
    Key::Name(name.to_string())
}

/// The address of the job record.
pub fn job_record_address() -> Address {
    Address {
        table: Table::Meta,
        key: named(JOB_RECORD),
    }
}

/// The address of the `corrections` row `name` ([`LINE_CORRECTIONS`], [`TEXT_CORRECTIONS`] or
/// [`FIX_RECORD`]).
pub fn corrections_address(name: &str) -> Address {
    Address {
        table: Table::Corrections,
        key: named(name),
    }
}

/// The step and part an `outputs` name addresses; `None` when it names no document a step writes.
pub fn parse_output(name: &str) -> Option<(StepName, Option<&'static str>)> {
    let (step, part) = match name.split_once('/') {
        Some((step, part)) => (step, Some(part)),
        None => (name, None),
    };
    let step = step.parse::<StepName>().ok()?;
    output_parts(step)
        .iter()
        .find(|listed| **listed == part)
        .map(|listed| (step, *listed))
}

#[cfg(test)]
#[path = "tests/keys.rs"]
mod tests;
