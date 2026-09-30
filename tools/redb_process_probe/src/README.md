# redb process probe source

The `redb-process-probe` command line, the one way it opens a database, the `hold` and `attempt`
commands, the matrix of scenarios, the filesystem lookup for the matrix header, and the rkyv
in-place check.

## Contents

```text
tools/redb_process_probe/src/
├── attempt.rs     one open with the counter it read and the repair it ran; a summary of many opens
├── filesystem.rs  the filesystem a folder lives on, from `/proc/self/mountinfo`
├── holder.rs      the `hold` command: keeps a database open, and optionally committing, until stdin ends
├── inplace.rs     the `inplace` command: an rkyv archive stored in redb and accessed where it lies
├── main.rs        the binary root: the command line, the dispatch and the pinned versions
├── matrix/        the `matrix` command: the scenarios, the holder children and the result table
├── open_mode.rs   read-write or read-only, the sharing mode, the redb builder, the outcome of an open
└── tests/         unit tests for the outcome and summary texts, the mount table and the in-place check
```

## How it works

`main.rs` parses the command line with clap and hands each command to its module. Every open goes
through `open_mode::open`, which builds a `redb::Builder` for the sharing mode, times the call,
and returns an `OpenOutcome` (opened, or failed with redb's Display and Debug texts) with the
handle. `attempt.rs` adds the counter row the handle reads and, when asked, how often redb called
the repair callback. `holder.rs` is the process on the other side: it opens, seeds the counter,
prints `ready` and holds. `matrix/` starts holders as children of this executable and records each
attempt as a table row. `inplace.rs` needs no second process.

```text
main ──▶ hold    ──▶ holder::run  ──▶ open_mode::open
     ├─▶ attempt ──▶ attempt::attempt ──▶ open_mode::open, attempt::read_counter
     ├─▶ matrix  ──▶ matrix::run ──▶ holder children + attempt::attempt ──▶ table
     └─▶ inplace ──▶ inplace::run ──▶ open_mode::builder, rkyv::access
```

## Public surface

- The binary `redb-process-probe`; no module is used outside this crate.

## Boundaries

- Depends on: `redb`, `rkyv`, `clap` and `anyhow`; this executable itself, as the `hold`
  children.
- Used by: nothing; it is the binary's source.
- Rules:
  - `open_mode::open` is the only place that opens a database for an attempt or a holder, so both
    sides open the file the same way;
  - `attempt` exits 0 whatever the open did, and `matrix` exits 2 only when a scenario could not
    run (the headers in `main.rs` and `matrix/mod.rs`).
