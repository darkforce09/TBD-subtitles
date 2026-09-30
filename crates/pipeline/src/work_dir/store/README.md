# Job store

The one handle of a job's database, `job.redb`, that a process holds: who owns it, the six tables
and the layout version of each, and rkyv rows written in one transaction and read back typed, in
place or as bytes.

## Contents

```text
crates/pipeline/src/work_dir/store/
├── mod.rs      `JobStore`: the open, the registry, `job.lock`, the layout check and transactions
├── records.rs  `StoreWrite` (put, reserve, remove, commit) and `StoreRead` (get, view, raw, keys)
├── tables.rs   one redb definition per table and `LAYOUT_VERSIONS`
└── tests/      unit tests for ownership, the shared handle, the layout check and the rows
```

## How it works

`JobStore::open` creates the job folder and opens `job.redb` read-write with a 256 MiB page cache
([measurement](/documentation/research/redb_large_transaction_memory.md)); a file a killed writer
left is repaired on that open, keeping every commit, and the repair is logged. Opens go through a
process-wide registry of weak handles keyed by the job folder, so a second open in the same
process returns the same `Arc`, and the last one dropped closes the database. redb locks the file
for the life of the handle: a second handle, in this process or another, fails at once with
`DatabaseAlreadyOpen` ([measurements](/documentation/research/redb_multi_process.md)). Then the
open reads `job.lock`: when it names this process, the handle it sees is one of ours still
closing on another thread, and the open retries every 20 ms for up to 1 s; otherwise it returns
`PipelineError::busy` with the pid `job.lock` names, or none. Only after an open succeeds does the
store write this process's pid to `job.lock` (through a part file); a store that closes removes
`job.lock` while it still names this process, after the database is closed, and it holds the
registry meanwhile, so a new open in the process never sees the file half closed.
`JobStore::open_existing`, for reading a job, does the same with a file that must exist, and
creates nothing when it does not.

Every fresh open runs one transaction: it reads the `TableLayouts` under the `meta` key `layout`,
drops each table whose stored version differs from `LAYOUT_VERSIONS` (or has none while the table
exists), creates the six tables and stores the current versions. There is no migration: a
dropped table's steps run again.

| Table | Key | Definition |
|---|---|---|
| `meta` | a name (`layout`) | `NamedTable` |
| `step_records` | a step name | `NamedTable` |
| `outputs` | a step name, or an engine or pass | `NamedTable` |
| `corrections` | a name | `NamedTable` |
| `frames` | (occurrence id, frame number) | `FramedTable` |
| `readings` | (occurrence id, frame number) | `FramedTable` |

`JobStore::write` begins the database's one write transaction (a second waits for it) as a
`StoreWrite`: `put` archives a value with rkyv, `reserve` lets a caller fill the value's bytes in
place (such as straight from a worker's pipe) and leaves no row when the fill fails, `remove`
takes a row out, and `commit` makes them visible at once. `JobStore::read` gives a `StoreRead`
snapshot: `get` checks and copies a row out, `view` checks the archive and reads it in place,
`raw` copies the bytes and `keys` lists a table. Each takes a `worker_channel::address` table and
key; a key whose kind is not its table's is an error. No step writes here yet.

## Boundaries

- Depends on: `redb` 4.3.0, `rkyv` (the format `job_model` pins), `job_model::store::TableLayouts`,
  `worker_channel::address::{Table, Key}`, `tracing`, and `super::{WorkDir, write_text}`.
- Used by: `crate::runner::run_job` and `crate::fix_it::fix_job`, which hold a `JobStore` while
  they run; `tools/visual_validation/src/pilot.rs`.
- Rules:
  - an open creates the six tables, the layout and `job.lock`
    (`an_open_creates_the_six_tables_the_layout_and_the_lock` in `tests/store.rs`);
  - every caller in a process shares one handle, and the last one closes it and removes
    `job.lock` (`every_caller_in_a_process_shares_one_handle_and_the_last_one_closes_it`);
  - a second handle is refused, and a foreign owner is busy with its pid or none, its `job.lock`
    untouched (`a_second_handle_is_refused_and_a_foreign_owner_is_busy`);
  - a changed layout version drops that table only, and a table without a stored version is
    dropped (`a_changed_layout_version_drops_that_table_and_keeps_the_others`,
    `a_table_without_a_stored_version_is_dropped`);
  - `open_existing` never creates a database or a folder
    (`opening_an_existing_store_never_creates_one`);
  - rows come back by `get` and `view`, frame rows in frame order, and a key of the wrong kind is
    an error (`a_step_record_comes_back_by_get_and_by_view`,
    `a_frame_row_comes_back_and_lists_in_frame_order`, `a_key_of_the_wrong_kind_is_an_error` in
    `tests/records.rs`);
  - a reserved row is filled in place and a failed fill leaves none, and nothing is visible before
    `commit` (`a_reserved_row_is_filled_in_place_and_a_failed_fill_leaves_none`,
    `a_removed_row_is_gone_and_an_uncommitted_write_changes_nothing`);
  - whoever changes a type stored in a table bumps that table's version in `LAYOUT_VERSIONS`
    (review).

## Related documentation

- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#process-ownership) —
  who owns `job.redb`, its tables and why a layout version drops a table.
- [redb across processes](/documentation/research/redb_multi_process.md) — the measured locking
  and repair behaviour the ownership rules follow.
