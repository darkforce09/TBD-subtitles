# Visual validation source

Commands and fail-closed evaluation for local visual pilots.

## Contents

```text
tools/visual_validation/src/
├── evaluate.rs      coverage, timing and geometry verdicts on a pilot job's typeset text
├── job_rows.rs      a finished job's source, probe, documents and frame shifts, from its database
├── main.rs          fetch, still recognition, clip, evaluation and probe commands
├── mask_probe.rs    stroke-mask and residue diagnostics on a finished job, and a whole-step rerun
├── pilot.rs         measured six-stage pilots using production workers and resume
├── scenarios.rs     synthetic Japanese credits, lyric, vertical and brief-text source fixtures
├── verify_probe.rs  the read-back check over a finished job, every frame's readings printed
└── tests/           evaluator regression cases
```

## How it works

Commands use production backends and write explicit outputs. `pilot.rs` runs the six on-screen text steps into a work directory's job database (`job.redb`) through the production resume, worker and task code, storing the probe, dialogue cues and shot changes it needs first. `job_rows.rs` reads a job back from that database, one read per call: its job record and probe, one on-screen text or replacement document, and the per-frame shifts of its `frames` rows; a job another process holds open is a busy error. Evaluation matches independent annotations to distinct observed occurrences in the stored `text_typeset` document and checks timing and geometry at source resolution. The mask and residue probes call the production `replace::mask::diagnose`, `replace::mask::extract` and `replace::inpaint::residue_share` on a finished job's documents, read from its database, and its mask and plate files and source video, on the CPU, and write only under the folder they are given. The verify probe runs the production `replace::verify::verify` with PP-OCRv5 on the GPU over the same documents, the `frames` rows and the patch files, and writes only the regions it read, under `--out`.

## Boundaries

- Depends on: `crates/pipeline` (the job store, resume, workers and tasks), `crates/stages`,
  `crates/inference`, `crates/job_model`, `crates/media_io`, `crates/subtitle_formats`,
  `crates/child_process` and `crates/worker_channel` (the `frames` table name); the crates.io
  crates `clap`, `anyhow`, `image`, `serde`, `serde_json` and `ureq`; FFmpeg.
- Used by: the `visual_validation` binary, run through `cargo run -p visual_validation`.
- Rules: source videos are read-only and annotation failures never become passing verdicts
  (`invalid_annotations_fail_closed_with_a_written_verdict`,
  `malformed_input_overwrites_a_previous_passing_verdict`).

## Related documentation

- [Validation tool](/tools/visual_validation/) — commands and configuration.
