# Probe matrix

The `matrix` command: the seven scenarios in which a second process opens a database another
process holds, and the Markdown table they fill.

## Contents

```text
tools/redb_process_probe/src/matrix/
├── holder_process.rs  a `hold` child of this executable: start, wait for `ready`, close or SIGKILL
├── mod.rs             the header, the order of the scenarios, the files, the exit code
├── scenarios.rs       the seven scenarios, each recording its observations as rows
├── table.rs           the rows as a Markdown table with escaped cells
└── tests/             unit tests for the table rendering and how a holder's end is described
```

## How it works

`mod.rs` prints the header, then runs the scenarios in `scenarios.rs` in order. They are an idle
writer, a busy writer, a reader, two readers, a crashed writer, the same process and a clean
handover. A scenario starts its holder through `holder_process.rs`, which runs this executable
as `hold` and waits up to 10 s for `ready`. It then makes its attempts in the matrix process and
ends the holder: it closes stdin and waits up to 5 s, killing the holder after that, or sends
SIGKILL in the crash scenario. A writing holder is checked to be running before and after the
attempts made against it and right before its SIGKILL. Every observation becomes a `table::Row`.
A holder that never becomes ready, a writing holder that has already exited (`holder exited
early: …`, with its own `failed: …` line) and a crash that SIGKILL did not cause are recorded
too, and each makes the exit code 2.

## Boundaries

- Depends on: `crate::attempt` (the attempts and their summary), `crate::holder` (the counter
  seed), `crate::open_mode`, `crate::filesystem`; this executable, as the `hold` children.
- Used by: `main.rs`, for the `matrix` command.
- Rules:
  - every holder child is ended and waited for before its scenario returns, on error paths too,
    and dropping a `HolderProcess` kills and reaps a child nothing reaped; every database file is
    removed at the end (the headers in `holder_process.rs` and `mod.rs`);
  - only SIGKILL counts as the crash (`only_sigkill_counts_as_killed` in
    `tests/holder_process.rs`);
  - a cell's text is verbatim but for `|` and line breaks (`pipes_and_line_breaks_cannot_break_a_cell`
    in `tests/table.rs`).
