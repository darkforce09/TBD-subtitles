//! What the job database keeps about its own tables: the layout version each was written with.
//!
//! **Role:** the value stored under the key `"layout"` of the job database's `meta` table, naming
//! every table and the layout version of the types it stores.
//!
//! **Position:** written and read by `pipeline::work_dir::store` when it opens a job's database.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** a table missing from `versions` was never written with a known layout; the map
//! is ordered, so the archive is the same for the same versions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The layout version of every table of a job database, by table name.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct TableLayouts {
    /// Table name (`meta`, `step_records` …) to the version of the layout its rows were written
    /// with.
    pub versions: BTreeMap<String, u32>,
}

#[cfg(test)]
#[path = "tests/archive.rs"]
mod archive_tests;
