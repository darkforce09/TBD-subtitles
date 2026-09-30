# Work directory

The job's [work directory](/documentation/glossary.md#work-directory): the path of every file a job
keeps, the job id, the default folder of all jobs, JSON and text written so a killed job never
leaves half a file, and the job store that owns the job's database, `job.redb`.

## Contents

```text
crates/pipeline/src/work_dir/
├── corrections.rs  `review.json` read and changed under its lock, and its digest
├── mod.rs          `WorkDir` and its paths, `job_id`, `default_root`, `gpu_lock_path`, JSON I/O
├── store/          `JobStore`: the one handle of `job.redb` a process holds, its tables and rows
└── tests/          unit tests for the job id, the JSON round trip and the corrections lock
```

## How it works

`WorkDir` names each file in one place: `job.json`, `job.redb`, `job.lock`, `probe.json`, the three 16 kHz
files under `audio/`, `shots.json`, `vad.json`, `asr/<engine>.json`, `sheet.json` and
`sheet.txt`, `sound_events.json`, `adjudication/first.json` and
`adjudication/redecode_<engine>.json`, `adjudicated.json`, `sound_cues.json`, `aligned.json`,
`review.json` and its lock `review.json.lock`, `reviewed.json`, Fix It's `fix.json` and
`fix/calls/`, `cues.json` and `cues_dropped_sounds.json`, `qc.json`, `report.md`, `output.json`, the empty
`claude-cwd/`, `backup/` and `logs/<step>.log`. `job_id` is the video's
file stem as a lowercase slug plus the first 8 hex digits of the SHA-256 of its full path.
`default_root` is `work/` in the app's data folder, and `gpu_lock_path` is `gpu.lock` beside it. `write_text` creates the folder, writes
`<name>.part` and renames it over `<name>`.

`store::JobStore` owns `job.redb`, the job database of the
[binary storage plan](/documentation/architecture/binary_storage_plan.md). One process at a time
has it open, read-write, and every caller in that process (the window's queue, its job threads,
Fix It, the background shot scan) shares one handle through a process-wide registry keyed by the
job folder. `job.lock` holds the owner's pid; it is written only after the open succeeds, so it
never names a process that does not own the database, and it goes when the last handle closes.
While another process owns the file, an open fails with the busy error kind naming the pid
`job.lock` holds. Every job database runs with a 256 MiB page cache, which bounds the memory of an
open write transaction. No step writes to the database yet: every step keeps writing its JSON
file, and `job.json` stays the step record.

`update_corrections` is the one way to change `review.json`: it takes an exclusive lock on
`review.json.lock`, reads the file as it is on disk, applies the change, and writes the file (or
removes it once empty) only when something changed. The window's line review and Fix It both go
through it, so neither writes over a correction the other made meanwhile. `corrections_digest` is
the SHA-256 of the file, which the review step's fingerprint covers.

## Boundaries

- Depends on: `serde`, `serde_json`, `sha2`, `job_model::StepName`,
  `inference::model_store::app_data_dir` for the default root, and, in `store/`, `redb`, `rkyv`,
  `job_model::store` and `worker_channel::address`.
- Used by: every other module of the crate; `crate::runner` and `crate::fix_it` for `JobStore`;
  `apps/tbd_subtitles/src/cli/` for `default_root`; the window's line review for
  `update_corrections`; `tools/visual_validation/` for `JobStore`.
- Rules:
  - a file that exists is complete, and no part file is left behind
    (`json_round_trips_and_leaves_no_part_file` in `tests/work_dir.rs`);
  - the job id depends only on the video's path
    (`a_job_id_is_a_slug_of_the_name_and_a_hash_of_the_path`);
  - two writers of the corrections at once lose no change
    (`two_writers_at_once_lose_no_change` in `tests/corrections.rs`);
  - one process holds a job's database, through one shared handle (the rules in
    `store/README.md`);
  - nothing is written outside the job's folder.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the
  layout of a job's work directory.
