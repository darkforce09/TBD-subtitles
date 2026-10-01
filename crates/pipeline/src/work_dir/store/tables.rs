//! The tables of `job.redb`: one redb definition per `worker_channel` table, and the layout
//! version of what each stores.
//!
//! **Role:** name each table's key and value types for redb, and declare the layout version each
//! table's rows are written with.
//!
//! **Position:** used by `store` to open and version the tables and by `records` to read and
//! write rows; the definitions are the ones a write that reserves its value in place uses too.
//!
//! **Signals and state:** none; constants.
//!
//! **Invariants:** every value is the bytes of an rkyv archive; the four text-keyed tables take a
//! `Key::Name`, `frames` and `readings` a `Key::Frame`; every table of `Table::ALL` has exactly
//! one definition and one layout version.

use redb::TableDefinition;
use worker_channel::address::Table;

/// A table keyed by a name, such as a step's name.
pub type NamedTable = TableDefinition<'static, &'static str, &'static [u8]>;

/// A table keyed by an on-screen text occurrence and a frame number.
pub type FramedTable = TableDefinition<'static, (&'static str, u64), &'static [u8]>;

pub const META: NamedTable = TableDefinition::new("meta");
pub const STEP_RECORDS: NamedTable = TableDefinition::new("step_records");
pub const OUTPUTS: NamedTable = TableDefinition::new("outputs");
pub const CORRECTIONS: NamedTable = TableDefinition::new("corrections");
pub const FRAMES: FramedTable = TableDefinition::new("frames");
pub const READINGS: FramedTable = TableDefinition::new("readings");

/// The key of the `meta` row that holds every table's layout version.
pub const LAYOUT_KEY: &str = "layout";

/// The layout version of every table's rows. Whoever changes a type stored in a table bumps that
/// table's version: a database whose stored version differs drops the table on its next open, and
/// the steps that wrote it run again ("Adding a field" in
/// `documentation/architecture/binary_storage_plan.md`).
pub const LAYOUT_VERSIONS: [(Table, u32); 6] = [
    (Table::Meta, 2),
    (Table::StepRecords, 1),
    (Table::Outputs, 2),
    (Table::Corrections, 1),
    (Table::Frames, 2),
    (Table::Readings, 2),
];

/// A table's redb definition, by the kind of key it takes.
#[derive(Clone, Copy)]
pub enum Definition {
    Named(NamedTable),
    Framed(FramedTable),
}

/// The redb definition of `table`.
pub fn definition(table: Table) -> Definition {
    match table {
        Table::Meta => Definition::Named(META),
        Table::StepRecords => Definition::Named(STEP_RECORDS),
        Table::Outputs => Definition::Named(OUTPUTS),
        Table::Corrections => Definition::Named(CORRECTIONS),
        Table::Frames => Definition::Framed(FRAMES),
        Table::Readings => Definition::Framed(READINGS),
    }
}
