# Work directory

The job's [work directory](/documentation/glossary.md#work-directory): the path of every file a job
keeps, the job id, the default folder of all jobs, and JSON and text written so a killed job never
leaves half a file.

## Contents

```text
crates/pipeline/src/work_dir/
├── corrections.rs  `review.json` read and changed under its lock, and its digest
├── mod.rs          `WorkDir` and its paths, `job_id`, `default_root`, `gpu_lock_path`, JSON I/O
└── tests/          unit tests for the job id, the JSON round trip and the corrections lock
```

## How it works

`WorkDir` names each file in one place: `job.json`, `job.lock`, `probe.json`, the three 16 kHz
files under `audio/`, `shots.json`, `vad.json`, `asr/<engine>.json`, `sheet.json` and
`sheet.txt`, `sound_events.json`, `adjudication/first.json` and
`adjudication/redecode_<engine>.json`, `adjudicated.json`, `sound_cues.json`, `aligned.json`,
`review.json` and its lock `review.json.lock`, `reviewed.json`, Fix It's `fix.json` and
`fix/calls/`, `cues.json` and `cues_dropped_sounds.json`, `qc.json`, `report.md`, `output.json`, the empty
`claude-cwd/`, `backup/`, `logs/<step>.log` and `steps/<step>.worker.json`. `job_id` is the video's
file stem as a lowercase slug plus the first 8 hex digits of the SHA-256 of its full path.
`default_root` is `work/` in the app's data folder, and `gpu_lock_path` is `gpu.lock` beside it. `write_text` creates the folder, writes
`<name>.part` and renames it over `<name>`.

`update_corrections` is the one way to change `review.json`: it takes an exclusive lock on
`review.json.lock`, reads the file as it is on disk, applies the change, and writes the file (or
removes it once empty) only when something changed. The window's line review and Fix It both go
through it, so neither writes over a correction the other made meanwhile. `corrections_digest` is
the SHA-256 of the file, which the review step's fingerprint covers.

## Boundaries

- Depends on: `serde`, `serde_json`, `sha2`, `job_model::StepName`, and
  `inference::model_store::app_data_dir` for the default root.
- Used by: every other module of the crate; `apps/tbd_subtitles/src/cli/` for `default_root`;
  the window's line review for `update_corrections`.
- Rules:
  - a file that exists is complete, and no part file is left behind
    (`json_round_trips_and_leaves_no_part_file` in `tests/work_dir.rs`);
  - the job id depends only on the video's path
    (`a_job_id_is_a_slug_of_the_name_and_a_hash_of_the_path`);
  - two writers of the corrections at once lose no change
    (`two_writers_at_once_lose_no_change` in `tests/corrections.rs`);
  - nothing is written outside the job's folder.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the
  layout of a job's work directory.
