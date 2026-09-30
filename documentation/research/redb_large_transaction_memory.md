**Status:** frozen record (2026-09-30)

# redb memory for a large uncommitted transaction

How much memory `redb` 4.3.0 holds while one write transaction stays open over a whole per-frame
table, the size of a `text_mask` output for a two-hour video at 60 fps, and what the commit costs.
Measured on 2026-09-30 with [`redb-process-probe txn-memory`](/tools/redb_process_probe/README.md)
(release build), on the host (Bazzite, kernel 7.2.4, i7-14700K, 31 GB RAM), with the database on
btrfs in the work root's folder (`~/.local/share/tbd-subtitles/redb-probe`). A Dressrosa 28 run
of the app encoded its localized video on the GPU at the same time. The findings serve the
[binary storage plan](/documentation/architecture/binary_storage_plan.md), whose per-frame tables
commit one step's rows in one transaction.

## Method

One `begin_write`, then for each row `insert_reserve((occurrence, frame), value_bytes)` into a
table `(&str, u64) -> &[u8]`, the slice filled with incompressible pseudo-random bytes; rows are
spread over 40 occurrence ids. `VmRSS` and `VmHWM` come from `/proc/self/status` before the
transaction, after each tenth of the rows, before the commit, after it, and after the database
is dropped. The cache is redb's page cache (`Builder::set_cache_size`; redb's default is 1 GiB).
"File" is the file's length after the commit; redb grows the file in regions, so it is an upper
bound on the pages in use.

## Results

432,000 rows (two hours at 60 fps):

| Value bytes | Payload | Cache | Peak RSS (HWM) | RSS before commit | Inserts | Commit | File |
|---|---|---|---|---|---|---|---|
| 2048 | 843.8 MiB | 1024 MiB | 556.8 MiB | 542.0 MiB | 4.19 s | 16.54 s | 2056 MiB |
| 2048 | 843.8 MiB | 256 MiB | 153.9 MiB | 144.5 MiB | 4.67 s | 25.05 s | 2056 MiB |
| 2048 | 843.8 MiB | 64 MiB | 52.4 MiB | 44.4 MiB | 3.70 s | 24.63 s | 2056 MiB |
| 1024 | 421.9 MiB | 256 MiB | 145.2 MiB | — | 1.89 s | 7.75 s | 1028 MiB |

With the default cache, RSS rose by about 180 MiB per 43,200 rows until 538 MiB (at 30 % of the
rows) and stayed there to the commit: the uncommitted rows are not held in memory beyond the
cache; redb writes dirty pages out to the file while the transaction is open. After the database
was dropped the process kept 470 MiB (the allocator's, not redb's).

## Conclusions

- **Memory is bounded by the cache, not by the transaction.** One transaction per step fits the
  8 GB limit at any video length; a 256 MiB cache holds a two-hour 60 fps table in 154 MiB, and a
  64 MiB cache in 52 MiB. The default 1 GiB cache peaks at 557 MiB here.
- **The commit costs seconds, not memory.** Writing out and syncing about 2 GiB took 17–25 s with
  the encode running beside it, on top of 4 s of inserts.
- **The file is larger than the payload.** 2,048- and 1,024-byte values both gave a file 2.4
  times the payload; per-frame rows should stay compact (run-length masks, not bitmaps) and
  large media stay files.

## Recommended stack

| Capability | Choice |
|---|---|
| Per-frame tables | one write transaction per step, as the plan says; no chunked commits needed |
| Page cache of a job database | set explicitly, 256 MiB: under 160 MiB for the largest table measured, and far below the 8 GB limit |

## Hard gaps

- The used part of the file was not measured, only its length; phase 5 measures `job.redb` on
  real per-frame data.
- The commit time was measured with a GPU encode running on the same machine and one disk; it is
  not a benchmark of the disk.

## Sources

[redb 4.3.0 on docs.rs](https://docs.rs/redb/4.3.0/redb/) ·
[`Builder::set_cache_size`](https://docs.rs/redb/4.3.0/redb/struct.Builder.html#method.set_cache_size)
