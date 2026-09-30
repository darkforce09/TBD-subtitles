**Status:** frozen record (2026-09-30)

# redb across processes

What `redb` 4.3.0 does when a second process opens a database file that another process holds,
read-write and read-only, idle and while it writes, after a crash, and within one process; and
whether an `rkyv` 0.8.18 archive is read in place from a redb value. Measured on 2026-09-30 with
[`redb-process-probe`](/tools/redb_process_probe/README.md) (`matrix` and `inplace`), in the
`claude-desktop` container (Debian 12) and on the host (Bazzite), on btrfs (the work root,
`~/.local/share/tbd-subtitles`) and on ext4 (`/run/media/system/Disk_2`). Versions are the
crates.io releases of that day: redb 4.3.0 (2026-09-14), rkyv 0.8.18. The findings serve the
[binary storage plan](/documentation/architecture/binary_storage_plan.md) and its
[decision entry](/documentation/decisions/storage.md).

"rw" is `Database::create`, "ro" is `ReadOnlyDatabase::open`. A holder is a child process that
opened the file and keeps it open; each attempt runs in a separate process. "Writing" means the
holder commits a counter in a loop with each write transaction left open for 20 ms.

## 1. How redb locks the file

redb 4.3.0 takes `fcntl` `F_OFD_SETLK` byte-range locks (open file description locks) on ranges
at offset 2^62 and above, as non-blocking try-locks. Because the lock belongs to the open file and
not to the process, a second open inside the same process conflicts like one from another
process. In the default mode (`ExclusiveWriter`) a read-write open takes the ranges exclusively
and a read-only open takes them shared. The feature `experimental-multiprocess` (which also turns
on the redb 5 API preview, `experimental-api-5`) adds `ConcurrencyMode` and
`Builder::set_concurrency_mode`; without that call the mode stays `ExclusiveWriter`.

## 2. Default build (`ExclusiveWriter`)

The same results in the container and on the host, on btrfs and on ext4; open times vary by
hundredths of a millisecond.

| Scenario | Holder | Attempt | Result |
|---|---|---|---|
| 1 | rw, idle | rw | fails in 0.06 ms: `DatabaseAlreadyOpen` ("Database already open. Cannot acquire lock.") |
| 1 | rw, idle | ro | fails in 0.02 ms: `DatabaseAlreadyOpen` |
| 2 | rw, writing | rw ×20 | 0 of 20 open; all `DatabaseAlreadyOpen` |
| 2 | rw, writing | ro ×20 | 0 of 20 open; all `DatabaseAlreadyOpen` |
| 3 | ro | ro | opens in 0.2 ms |
| 3 | ro | rw | fails: `DatabaseAlreadyOpen` |
| 4 | ro | second ro holder | both stay open |
| 5 | rw, writing, killed with SIGKILL | rw with a repair callback | opens in 3.4–3.9 ms; repair runs (3 callback calls); the last committed counter (13 or 14) is there |
| 5 | killed, then repaired by a rw open | ro | opens; same counter |
| 5 | rw, writing, killed with SIGKILL | ro first | fails: `RepairAborted` ("Database repair aborted.") until a rw open repairs the file |
| 6 | rw handle in the attempting process itself | rw, ro | both fail: `DatabaseAlreadyOpen` |
| 7 | rw, exited cleanly | rw, ro | both open (rw 1.0 ms, ro 0.2 ms) |

Across the container boundary the locks hold the same way, since both share the host kernel: with
the container holding rw and writing, a host rw and a host ro open both failed with
`DatabaseAlreadyOpen`; with the host holding ro, a container rw open failed and a container ro open
opened.

A refused open fails at once (under 0.1 ms) and never waits; a caller that wants to wait must retry.

## 3. Experimental build (`experimental-multiprocess`)

Measured in the container and on the host (btrfs); the same results in both.

| Mode | What a second process can do |
|---|---|
| `ExclusiveWriter` (default) | as in section 2 |
| `SingleWriter` | ro opens while a writer holds the file (scenario 2: 20 of 20, reading counters that the writer commits); a second rw fails with `DatabaseAlreadyOpen`, also within one process; a rw opens while only readers hold it |
| `MultiWriter` | every rw and ro open succeeds; a rw open waits for the current write transaction (16–37 ms with 20 ms transactions); after a SIGKILL a rw open reports no repair and finds the last commit |

In every mode a read-only open of a file left by a killed writer fails with `RepairAborted` until a
read-write open repairs it. Readers must open in the writer's mode.

## 4. rkyv archives read in place

`inplace` archives a record (a string, a `u128`, an `f64`, an `Option<f64>`, a `Vec<String>`, a
`BTreeMap<String, u64>`; 232 bytes) with rkyv's `unaligned` feature, stores it as a `&[u8]` value,
reopens the database and calls `rkyv::access` on the slice redb returns, with no copy: the fields
read back and the value deserialises equal to the original. The same holds on a copy shifted to an
odd address. It printed `inplace: ok` in the container and on the host, on btrfs and on ext4, in
both builds.

## Recommended stack

| Capability | Choice |
|---|---|
| Job database | redb 4.3.0, default features, `ExclusiveWriter`: one process owns `job.redb`; nothing else opens it while that process lives |
| Values | rkyv 0.8.18 with `unaligned`, read in place from redb's value slice |
| Reading another process's job | only after it exits; a read-only open then works, and several readers may share it |
| After a crash | the next owner opens read-write, which repairs the file; read-only opens fail until then |

`SingleWriter` would let the window read a job while a terminal run writes it, but it is
experimental and part of the redb 5 API preview, so the plan does not rely on it.

## Hard gaps

- **No reader beside a writer on stable redb:** the window cannot open a job that a
  `tbd-subtitles process` run from a terminal owns; it shows the job as busy until that process
  exits.
- **No waiting open:** a refused open returns at once; the owner of a lock is found through
  `job.lock`, not through redb.

## Sources

[redb changelog](https://github.com/cberner/redb/blob/master/CHANGELOG.md) ·
[redb 4.3.0 on docs.rs](https://docs.rs/redb/4.3.0/redb/) ·
[rkyv 0.8.18 on docs.rs](https://docs.rs/rkyv/0.8.18/rkyv/)
