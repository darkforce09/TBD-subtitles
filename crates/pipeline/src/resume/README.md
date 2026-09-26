# Resume

Whether a step's recorded output can be reused, and the lock that keeps two runs off one job. A
step is reused when the job record holds its current fingerprint and every file it leaves exists.

## Contents

```text
crates/pipeline/src/resume/
├── mod.rs  `fingerprint`, `is_valid`, `stale_steps`, and `lock` with its `JobLock` guard
└── tests/  unit tests for reuse, invalidation by settings, video and upstream steps, and the lock
```

## How it works

A fingerprint is the SHA-256 of a JSON value holding the step's name, its revision, the settings it
reads (`graph::settings`), the video's path, size and modification time when the step reads the
video itself, and the fingerprint and finish time of each step it reads. Re-running a step gives it
a new finish time, so every step that reads it gets a new fingerprint and runs again. `lock`
writes this process's pid to `job.lock`; a lock whose pid still runs refuses the job, and a lock
whose pid is gone is taken over. Dropping the `JobLock` removes the file. `stale_steps` lists, in
order, the steps a run would do now: each step that is not valid and each step that reads one of
them.

## Boundaries

- Depends on: `crate::graph` (inputs, revision, settings, outputs), `crate::work_dir`, `job_model`
  (`StepName`, `JobRecord`), `serde_json`, `sha2`, and `/proc/<pid>` for a lock's owner.
- Used by: `crate::runner`, before each step and when a job starts.
- Rules:
  - a missing output file reruns its step, and a part file is not an output
    (`a_finished_step_with_its_files_is_reused_and_a_missing_file_reruns_it` in
    `tests/resume.rs`);
  - a rerun step invalidates every step that reads it and no other
    (`a_rerun_upstream_step_invalidates_every_step_that_reads_it`);
  - a setting changes only the steps that read it, and the video's identity only the steps that
    read the video
    (`a_setting_changes_only_the_steps_that_read_it_and_the_video_changes_the_first`);
  - a live lock refuses a second run and a dead one is taken over
    (`a_live_lock_refuses_a_second_run_and_a_dead_one_is_taken_over`).
  - the stale steps are the invalid ones and every step that reads them
    (`the_stale_steps_are_the_invalid_ones_and_everything_that_reads_them`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — when a
  stage is skipped and how deleting an output reruns it.
