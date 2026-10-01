# Work directory

The job's [work directory](/documentation/glossary.md#work-directory): the path of every file a job
keeps, the job id, the default folder of all jobs, the files outside the database written
so a killed job never leaves half of one, the job store that owns the job's database, `job.redb`,
and the owner's corrections kept in it.

## Contents

```text
crates/pipeline/src/work_dir/
├── corrections.rs  the line corrections, the text corrections and Fix It's record in the store
├── mod.rs          `WorkDir` and its paths, `job_id`, `default_root`, `gpu_lock_path`, file writes
├── store/          `JobStore`: the one handle of `job.redb` a process holds, its tables and rows
└── tests/          unit tests for the job id, the part-file writes and the corrections
```

## How it works

`WorkDir` names each path in one place: `job.redb`, `job.lock`, the three 16 kHz files under
`audio/`, the replacement folders `visual/masks`, `visual/plates` and `visual/patches`,
`sheet.txt`, `fix/calls/`, `report.md`, the empty `claude-cwd/`, `backup/` and
`logs/<step>.log`. Every step document lives in `job.redb`; the files here are the large media
its rows name, the caches, the logs and the two files written for reading. `job_id` is the
video's file stem as a lowercase slug plus the first 8 hex digits of the SHA-256 of its full
path. `default_root` is `work/` in the app's data folder, and `gpu_lock_path` is `gpu.lock`
beside it. `write_text` (and `write_json`, for Fix It's call cache and the visual validation
tool's reports) creates the folder, writes `<name>.part` and renames it over `<name>`, and
`sync_file` flushes a file a step wrote before the row that names it commits.

`store::JobStore` owns `job.redb`, the job database of the
[binary storage plan](/documentation/architecture/binary_storage_plan.md). One process at a time
has it open, read-write, and every caller in that process (the window's queue, its job threads,
Fix It, the background shot scan, the window's readers) shares one handle through a process-wide
registry keyed by the job folder. `job.lock` holds the owner's pid; it is written only after the
open succeeds, so it never names a process that does not own the database, and it goes when the
last handle closes. While another process owns the file, an open fails with the busy error kind
naming the pid `job.lock` holds. The job record (`meta/job_record`) and every step's record
(`step_records/<step>`) live there; `load_job_record`, `load_step_records`, `read_job` and
`read_stored` read them for callers outside the runner.

`corrections.rs` keeps the owner's corrections in the store's `corrections` table:
`update_corrections` changes the line corrections (`corrections/lines`) and
`update_text_corrections` the on-screen text corrections (`corrections/text`), each in one write
transaction that reads the row, applies the change and writes it back (or removes it once empty)
only when something changed. The window's line review and Fix It both go through it, so neither
writes over a correction the other made meanwhile. `corrections_digest` is the SHA-256 of the
stored archive, which Fix It compares with the job record's; `resume` hashes the rows in the
fingerprints of the steps that read them. `read_fix_record` and `put_fix_record` keep Fix It's
record of its runs under `corrections/fix`, which no step rerun clears.

## Boundaries

- Depends on: `serde`, `serde_json`, `sha2`, `job_model`, `inference::model_store::app_data_dir`
  for the default root, and, in `store/`, `redb`, `rkyv`, `job_model::store` and
  `worker_channel::address`.
- Used by: every other module of the crate; `crate::runner`, `crate::resume` and `crate::fix_it`
  for `JobStore` and the corrections; `apps/tbd_subtitles/src/cli/` for `default_root` and
  `dump`; the window's queue (`job_queue/services/video_files`, `queue_store`, `time_left`),
  report, line review, text review and Fix It for the job's rows and the corrections;
  `tools/visual_validation/` for `JobStore` and `read_stored` (`job_rows.rs`).
- Rules:
  - a file that exists is complete, and no part file is left behind
    (`json_round_trips_and_leaves_no_part_file` in `tests/work_dir.rs`);
  - the job id depends only on the video's path
    (`a_job_id_is_a_slug_of_the_name_and_a_hash_of_the_path`);
  - writers of the corrections at once lose no change, and the last correction taken out removes
    the row (`writers_at_once_lose_no_change`,
    `a_change_is_stored_and_the_last_one_taken_out_removes_the_row` in `tests/corrections.rs`);
  - one process holds a job's database, through one shared handle (the rules in
    `store/README.md`);
  - nothing is written outside the job's folder.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the
  layout of a job's work directory.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#tables-in-jobredb) —
  the tables the store holds.
