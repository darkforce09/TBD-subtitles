# Resume

Whether a step's stored output can be reused. A step is reused when its step record holds its
current fingerprint, every document it writes is stored and every file its rows name exists. Keeping two runs off one job is the job
store's work (`crates/pipeline/src/work_dir/store/`), not this module's.

## Contents

```text
crates/pipeline/src/resume/
├── mod.rs  `fingerprint`, `is_valid` and `stale_steps`, each over one snapshot of the job's store
└── tests/  unit tests for reuse and invalidation by settings, video and upstream steps
```

## How it works

A fingerprint is the SHA-256 of a JSON value holding the step's name, its revision, the settings it
reads (`graph::settings`), the video's path, size and modification time when the step reads the
video itself, the stored layout version of every table, and the fingerprint and finish time of
each step it reads, from `step_records`. Re-running a step gives it a new finish time, so every
step that reads it gets a new fingerprint and runs again. The review's fingerprint also covers the
digest of the stored line corrections, and reading, translation and review, with on-screen text
on, the stored text corrections (all of them for the review, the occurrences to retry for the
other two), besides the model files and the reference folder. With a sign library, the translation's
fingerprint also covers the digest of the signs its tracked occurrences match, and the
composition's (with the localized video on) those its reviewed occurrences match
(`library::signs::digest`: key, English, confidence and style); a job no sign of another job
matches carries no such key, so an empty library changes no fingerprint. `is_valid` also needs
every key `keys::output_keys` gives the step and `work_dir::store::files::named_files` present: the files
the step's rows name, the same list the store's orphan cleanup keeps.
`stale_steps` lists, in order, the steps a run would do now: each step that is not valid and each
step that reads one of them.

## Boundaries

- Depends on: `crate::graph` (inputs, revision, settings, the corrections a step reads),
  `crate::library` (the signs a step matches),
  `crate::work_dir` (the store, its keys, `files` and the corrections' digest), `job_model`
  (`StepName`, `JobRecord`, `TableLayouts`), `serde_json` and `sha2`.
- Used by: `crate::runner`, before each step and when a job starts;
  `tools/visual_validation/`.
- Rules:
  - a missing document or file reruns its step, and a part file is not an output
    (`a_finished_step_with_its_rows_and_files_is_reused_and_a_missing_one_reruns_it` in
    `tests/resume.rs`), and a missing crop reruns the visual steps alone
    (`a_missing_visual_crop_invalidates_the_visual_steps_without_repeating_audio`);
  - the corrections reach only the steps that read them, and a changed table layout every step
    (`the_line_corrections_reach_the_review_step_alone`,
    `the_text_corrections_reach_reading_translation_and_review_alone`,
    `a_changed_table_layout_changes_every_fingerprint`);
  - a rerun step invalidates every step that reads it and no other
    (`a_rerun_upstream_step_invalidates_every_step_that_reads_it`);
  - a setting changes only the steps that read it, and the video's identity only the steps that
    read the video
    (`a_setting_changes_only_the_steps_that_read_it_and_the_video_changes_the_first`);
  - the stale steps are the invalid ones and every step that reads them
    (`the_stale_steps_are_the_invalid_ones_and_everything_that_reads_them`);
  - only a matched sign of another job reaches the translation and composition fingerprints, and
    a changed sign changes them
    (`only_a_matched_sign_of_another_job_reaches_the_translation_and_composition_fingerprints`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — when a
  stage is skipped and how deleting an output reruns it.
