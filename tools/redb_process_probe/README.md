# redb process probe

The `redb_process_probe` binary (`redb-process-probe`): a test program that shows what redb 4.3.0
does when a second process opens a database file that another process holds open or is writing,
read-write and read-only, that an rkyv archive can be read in place from a redb value slice, and
how much RAM redb holds for one large uncommitted write transaction. A developer runs it in the
container and on the host; its findings go into the redb research notes.

## Contents

```text
tools/redb_process_probe/
├── Cargo.toml  the `redb_process_probe` binary package, the `multiprocess` feature, each dependency's reason
└── src/        the command line, the redb opens, the holder, the matrix scenarios, the in-place check, the transaction memory run
```

## How it works

Every open goes through one function (`src/open_mode.rs`): read-write is `Builder::create`,
which creates a missing file, and read-only is `Builder::open_read_only`, both from a
`redb::Builder` carrying the sharing mode. An open that fails is reported with redb's own
Display and Debug texts, such as
`Database already open. Cannot acquire lock. | DatabaseAlreadyOpen`.

`matrix` runs each scenario against a fresh database file. It starts this same executable again
as a `hold` child, waits for the child's `ready` line, makes its attempts from the matrix process,
which is a separate process, and then closes the child's stdin or kills it. The scenarios are:
an idle writer; a writer that keeps committing, with each write transaction open about 20 ms (20
attempts in each mode); a read-only holder; two read-only holders at once; a writer killed by
SIGKILL, then a read-write open with a repair callback and a read-only open, first on the repaired
file and then on a second killed file; two opens in one process; and a writer that exits cleanly
first.

```text
matrix ──spawn──▶ redb-process-probe hold --db F --mode rw|ro [--writing] --sharing S
   │                 └─ opens F, prints "ready", holds (and commits) until stdin ends
   ├─ attempt rw / ro on F, in the matrix process ──▶ one table row each
   └─ close stdin (or SIGKILL) and wait ──▶ a row with how the holder ended
```

`txn-memory` runs in one process. It opens one database with the page cache set by
`--cache-mib` (`Builder::set_cache_size`), begins one write transaction, and inserts `--rows` rows
into a table `frames` of `(&str, u64)` to `&[u8]`, shaped like the per-frame table of a two-hour
60 fps video: the key is `(occ-NNNN, frame)` with the frame spread over `--occurrences` ids in
turn, and each value is `--value-bytes` long, reserved with `insert_reserve` and filled in place
with xorshift bytes that do not compress. It reads `VmRSS` and `VmHWM` from `/proc/self/status`
before the transaction, after each tenth of the rows, before the commit, after the commit and after
the database is dropped, and times the inserts and the commit.

### What redb 4.3.0 does underneath

These are facts from the redb 4.3.0 source, which the matrix then puts to the test:

- **Locks.** On Linux redb takes open-file-description byte-range locks (`fcntl` with
  `F_OFD_SETLK`) on bytes far past the file's data. They belong to the open file, not the
  process, so a second open in the same process conflicts just as one from another process does.
  The kernel releases them when a killed process's file closes.
- **Default mode, `ConcurrencyMode::ExclusiveWriter`.** A writer (`Database::create`,
  `Database::open`, `Builder::create`, `Builder::open`) locks the whole lock range exclusively. A
  reader (`ReadOnlyDatabase::open`, `Builder::open_read_only`) locks it shared. So readers can
  share the file with each other, and any conflicting open fails at once with
  `DatabaseError::DatabaseAlreadyOpen`; nothing waits.
- **Unclean files.** A writer that dies mid-transaction leaves the file needing repair. The next
  writer's open repairs it, calling the callback set with `Builder::set_repair_callback` at least
  once; the callback may call `RepairSession::abort`. A read-only open cannot repair, so it
  returns `DatabaseError::RepairAborted`.
- **The `experimental-multiprocess` feature** (`experimental-multiprocess = [std,
  experimental-api-5]`). It adds `Builder::set_concurrency_mode` and exports
  `redb::ConcurrencyMode` with two more modes:
  - `SingleWriter`: one writing process holds a writer byte, and any number of read-only
    processes follow its commits.
  - `MultiWriter`: any number of processes open the file read-write, and each write transaction
    waits for the writer byte, one at a time. A read-write open waits for that byte too while
    another process has a write transaction open.

  The feature also turns on the redb 5 API preview. Under it, `ReadableTable::get` must be
  called through the trait, so the probe always calls it that way. Readers must use the same mode
  as the writer.

## Getting started

Run these from the repository root. The probe needs no GPU and no FFmpeg, so it runs the same in
the container and on the host:

```bash
export CARGO_TARGET_DIR=/run/media/system/Disk_2/Projects/cargo-targets/main
cargo build -p redb_process_probe
$CARGO_TARGET_DIR/debug/redb-process-probe matrix --dir /tmp/redb-probe
$CARGO_TARGET_DIR/debug/redb-process-probe inplace --dir /tmp/redb-probe
$CARGO_TARGET_DIR/debug/redb-process-probe txn-memory --dir /tmp/redb-probe --rows 20000   # a quick look
distrobox-host-exec /run/media/system/Disk_2/Projects/cargo-targets/main/debug/redb-process-probe matrix --dir ~/.local/share/tbd-subtitles/redb-probe
cargo build --release -p redb_process_probe   # txn-memory measures an optimized build
distrobox-host-exec /run/media/system/Disk_2/Projects/cargo-targets/main/release/redb-process-probe txn-memory --dir ~/.local/share/tbd-subtitles/redb-probe
cargo build -p redb_process_probe --features multiprocess   # the experimental sharing modes
cargo test -p redb_process_probe
```

`matrix` prints a header naming the versions, the feature, the sharing mode, the folder and its
filesystem, then the table. It takes about a second. The folder matters, because file locks can
behave differently on tmpfs, ext4, btrfs and overlay filesystems.

## Configuration

- Cargo feature `multiprocess` (off by default): builds redb with `experimental-multiprocess`.
  With it, `--sharing` accepts `single-writer` and `multi-writer`, and the default becomes
  `multi-writer`. Without it, only `exclusive-writer` is accepted. `src/open_mode.rs` reads it.
- No environment variables and no files beyond the database files it is pointed at,
  `/proc/self/mountinfo` and `/proc/self/status`.

## Public surface

- The binary `redb-process-probe` and the commands below; no library.

## Commands

Each command is `redb-process-probe <command> [options]`. `--sharing
exclusive-writer|single-writer|multi-writer` is optional for `hold`, `attempt` and `matrix`. A
usage error exits 2, as does a folder that cannot be created.

### hold

- Synopsis: `redb-process-probe hold --db <DB> --mode <rw|ro> [--writing] [--sharing <SHARING>]`
- Does: opens the database; read-write creates it when missing, with a table `probe` holding one
  counter row. It prints `ready`, then keeps the database open until stdin reaches end of file.
  With `--writing` (rw only), it loops: begin a write transaction, add 1 to the counter, sleep
  about 20 ms with the transaction still open, commit.
- Exit codes: 0 stdin ended and the database closed; 1 the open, the counter seed or a write
  failed, printed on stdout as `failed: <Display> | <Debug>` (before `ready` for the open and the
  seed, after it for a write); 2 usage, including `--writing` with `--mode ro`.
- Example: `redb-process-probe hold --db /tmp/redb-probe/a.redb --mode rw --writing`

### attempt

- Synopsis: `redb-process-probe attempt --db <DB> --mode <rw|ro> [--sharing <SHARING>]`
- Does: opens the database once and prints one line, either `opened in <ms> ms; counter=<n>`
  (or `counter=missing`) or `failed in <ms> ms: <Display> | <Debug>`.
- Exit codes: 0 whatever the open did, since the outcome is the data; 2 usage.
- Example: `redb-process-probe attempt --db /tmp/redb-probe/a.redb --mode ro`

### matrix

- Synopsis: `redb-process-probe matrix --dir <DIR> [--sharing <SHARING>]`
- Does: runs the seven scenarios with fresh database files in the folder, which it creates when
  missing. It prints the header and a table `| scenario | holder | attempt | result |` with the
  exact texts, then removes its files.
- Exit codes: 0 the matrix ran, whatever the results; 2 it could not run, such as a holder that
  never printed `ready`, a writing holder that exited before or during the attempts made against
  it (a row `holder exited early: …`), or a crash-scenario holder that SIGKILL did not end, with
  the reasons listed under the table.
- Example: `redb-process-probe matrix --dir /tmp/redb-probe`

### inplace

- Synopsis: `redb-process-probe inplace --dir <DIR>`
- Does: archives a record shaped like a job's step record with `rkyv::to_bytes` and stores it as a
  `&[u8]` value. It reopens the database and calls `rkyv::access` on the `AccessGuard`'s slice
  where it lies. It prints the slice's address modulo 16 and the fields it read, deserializes,
  and compares with the original. It does the same on a copy that starts at an odd address,
  failing if the copy does not, then prints `inplace: ok` and removes the file.
- Exit codes: 0 `inplace: ok`; 1 a step failed, printed as `inplace: failed: <error>`; 2 usage,
  or a folder that cannot be created.
- Example: `redb-process-probe inplace --dir /tmp/redb-probe`

### txn-memory

- Synopsis: `redb-process-probe txn-memory --dir <DIR> [--rows <N>] [--value-bytes <N>]
  [--cache-mib <N>] [--occurrences <N>] [--keep]`
- Does: measures the RAM redb holds for one large uncommitted write transaction. It removes
  `<DIR>/txn-memory.redb` if present, opens it with a page cache of `--cache-mib` MiB (default
  1024, redb's own default), and inserts `--rows` rows (default 432000, two hours at 60 fps) of
  `--value-bytes` bytes each (default 2048) under keys `(occ-NNNN, frame)` spread over
  `--occurrences` ids (default 40), all in one write transaction that commits once at the end. It
  prints one line per sample, `rows <n> rss <MiB> hwm <MiB> (<stage>)`, taken before the
  transaction, after each tenth of the rows, before the commit, after the commit and after the
  database is dropped, then a summary block: rows, value bytes, payload MiB, cache MiB, insert s,
  commit s, file MiB, peak hwm MiB, rss before commit and rss after commit. It removes the file
  at the end unless `--keep`. The folder must not be on tmpfs, whose pages count against RAM, nor
  on a nearly full disk: the default run writes about 1 GiB of values.
- Exit codes: 0 the summary printed; 1 the measurement failed, printed as
  `txn-memory: failed: <error>`; 2 usage, or a folder that cannot be created.
- Example: `redb-process-probe txn-memory --dir ~/.local/share/tbd-subtitles/redb-probe`

## Boundaries

- Depends on: `redb` 4.3.0 (pinned exactly) and `rkyv` 0.8.18 with `unaligned`, as the job model
  uses it; `clap` and `anyhow`; no workspace crate. The only program it starts is itself
  (`std::env::current_exe`), as the `hold` children. That is not an external program, so the
  program rule is untouched.
- Used by: a developer checking redb's multi-process behaviour. Nothing depends on it.
- Rules:
  - the tool depends on no workspace crate, as the tool table in
    `tools/repo_gates/src/layout.rs` lists it (`cargo gates crate-layering`);
  - `REDB_VERSION` in `src/main.rs` equals the `redb` pin in `Cargo.toml`, and a refused open is
    reported with redb's own texts, never paraphrased (the header of `src/main.rs`);
  - every holder child is waited for, and every database file the probe makes is removed, except
    the one `txn-memory --keep` is asked to leave;
  - `txn-memory` puts every row in one write transaction and fills each value in place through
    `insert_reserve`, and reads memory only from `/proc/self/status`.

## Related documentation

- [redb across processes](/documentation/research/redb_multi_process.md) — the matrix results in
  the container and on the host.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md) — the plan that relies
  on these findings.
