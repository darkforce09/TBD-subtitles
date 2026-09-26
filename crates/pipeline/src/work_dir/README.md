# Work directory

The job's [work directory](/documentation/glossary.md#work-directory): the path of every file a job
keeps, the job id, the default folder of all jobs, and JSON and text written so a killed job never
leaves half a file.

## Contents

```text
crates/pipeline/src/work_dir/
├── mod.rs  `WorkDir` and its paths, `job_id`, `default_root`, `read_json` and the part-file writes
└── tests/  unit tests for the job id and the JSON round trip
```

## How it works

`WorkDir` names each file in one place: `job.json`, `job.lock`, `probe.json`, the three 16 kHz
files under `audio/`, `shots.json`, `vad.json`, `asr/<engine>.json`, `sheet.json` and
`sheet.txt`, `sound_events.json`, `adjudication/first.json` and
`adjudication/redecode_<engine>.json`, `adjudicated.json`, `sound_cues.json`, `aligned.json`,
`cues.json` and `cues_dropped_sounds.json`, `qc.json`, `report.md`, `output.json`, the empty
`claude-cwd/`, `backup/`, `logs/<step>.log` and `steps/<step>.worker.json`. `job_id` is the video's
file stem as a lowercase slug plus the first 8 hex digits of the SHA-256 of its full path.
`default_root` is `work/` in the app's data folder. `write_text` creates the folder, writes
`<name>.part` and renames it over `<name>`.

## Boundaries

- Depends on: `serde`, `serde_json`, `sha2`, `job_model::StepName`, and
  `inference::model_store::app_data_dir` for the default root.
- Used by: every other module of the crate; `apps/tbd_subtitles/src/cli/process_command.rs` for
  `default_root`.
- Rules:
  - a file that exists is complete, and no part file is left behind
    (`json_round_trips_and_leaves_no_part_file` in `tests/work_dir.rs`);
  - the job id depends only on the video's path
    (`a_job_id_is_a_slug_of_the_name_and_a_hash_of_the_path`);
  - nothing is written outside the job's folder.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the
  layout of a job's work directory.
