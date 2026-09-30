**Status:** live

# Binary storage: redb and rkyv

The proposed move of every job's step outputs from loose JSON files into one embedded `redb`
database per job, with values archived by `rkyv`, and of approved signs into one library shared
by every episode. It is a proposal the owner has not approved; nothing here is built.

## Why

JSON files served the prototype: every step writes `<name>.part` and renames it, and a step is
skipped while its file is valid. The next quality steps need data that grows with the video:

- a position, a follow score and an erase mask for every frame of every sign, not one mask per
  sign taken on its keyframe;
- the read-back check's readings for every sampled frame;
- videos up to two hours at 60 fps: about 432,000 frames, where a PNG per sign per frame means
  hundreds of thousands of files and a JSON document per step has to be parsed whole to read one
  frame.

`rkyv` archives are read in place, so a step reads one frame's record without parsing the rest;
`redb` gives keyed tables, atomic transactions (a crash never leaves half an output) and a rerun
that clears a step and everything after it in one transaction. A library shared by episodes lets
later episodes reuse signs already approved (title cards, name cards, recurring locations) instead
of erasing, lettering and asking Claude again.

## Goals

1. **Atomic steps.** A step's output and its record commit in one transaction or not at all.
2. **Per-frame data.** Keyed tables hold data per occurrence and frame, read in place.
3. **Safe reruns.** `--rerun <step>` removes that step and every step that reads it, in one
   transaction.
4. **Readable.** `tbd-subtitles dump` prints any record as JSON.
5. **No regression.** Each phase leaves Dressrosa 11 and 28 with identical subtitle files and
   the same approved replacements.

## Process ownership

`redb` is expected to let one process open a database file at a time, a second open failing;
phase 1 confirms it against the pinned release before anything depends on it. The pipeline runs
GPU steps in separate worker processes (law 7), and the window reads a job while it runs, so:

- **The runner owns `job.redb`.** Only the process running the job opens it, for the whole job.
- **Workers never open it.** The runner hands a worker its inputs (as today: arguments, stdin, or
  files it writes for the worker) and the worker returns its output on stdout or in a file under
  `work/<job>/exchange/`; the runner validates it and commits it.
- **The window reads through its runner.** The window runs jobs in its own process and shares
  that process's handle. When another process owns a job (a `tbd-subtitles process` run from a
  terminal), the window shows the job as busy and opens it after that process exits, and writes
  corrections only then; `review.json.lock` and Fix It's writes move to the same rule.
- **Fix It and corrections** write through the owning process in one transaction each.

## Storage layout

```text
work/<job_id>/
├── job.redb      every step's output, the job record, step records and per-frame tables
├── job.lock      the process that owns job.redb
├── exchange/     worker inputs and outputs in transit; emptied when a job is opened
├── audio/        16 kHz PCM streams (large files, referenced by records)
├── visual/       keyframe stills, crops, plates and patches (referenced by records)
├── logs/         step logs
└── report.md     the human report

library.redb      beside work/: approved signs shared by every episode
```

A file outside the database is written and synced before the record that names it commits. When
a job is opened, files no record names are removed and `exchange/` is emptied.

## Tables in `job.redb`

| Table | Key | Value |
|---|---|---|
| `meta` | `"job_record"`, `"layout"` | the job record (video, settings, models); the layout version of every table |
| `step_records` | step name (`probe_decode` … `localized_video`) | revision, fingerprint of its inputs, finished time, measures (today `job.json` steps and `steps/<step>.worker.json`) |
| `outputs` | step name, plus an engine or pass where a step writes several (`asr_parakeet`, `redecode_whisper`) | the step's output document (today `probe.json`, `vad.json`, `visual/text_review.json` …) |
| `corrections` | `"lines"`, `"text"` | the owner's corrections (today `review.json`, `visual/corrections.json`) |
| `frames` | (occurrence id, frame number) | quad, follow score and shift, mask as run-length rows, plate id |
| `readings` | (occurrence id, frame number) | the read-back check's Japanese found, English read, similarity, verdict |

Values are `rkyv` archives of the `job_model` types. `frames` and `readings` are new: they are
what per-frame methods (a temporal erase mask, per-frame following, per-frame approval) write.

## Adding a field

`serde` fills a missing field with its default; `rkyv` does not, and nothing here tries to. Each
table's layout has a version in `meta`, part of every step's fingerprint: a change to a type bumps
it, and every step whose stored layout differs is stale and reruns. There are no migrations, no
importer and no JSON read path: the app is in beta, jobs from before the database are deleted or
rerun, and Git keeps the old code.

## Resume and reruns

Law 6 becomes: *each step commits its output and its record in one transaction, and is skipped
while its record's revision and input fingerprint are current.* A rerun of a step deletes that
step's output and record and those of every step that reads it (the graph in
`crates/pipeline/src/graph/`), plus their `frames` and `readings` rows, in one transaction; files
they named become unreferenced and are removed on the next open. A decision entry records the
new law, and CLAUDE.md changes with it.

## Library shared by episodes

`library.redb` holds approved signs keyed by the normalised Japanese and a perceptual hash of the
sign's keyframe crop: the English, the lettering style, and the patch and mask of the approved
replacement. A later occurrence whose crop matches starts from the stored translation and
lettering and is still erased, composed and read back in its own frames. The window's Settings
shows the library's size and clears it. Only the running job writes to it, one sign per
transaction; readers open it read-only only when no job runs, otherwise through the runner.

## Phases

Each phase passes `cargo fmt`, `clippy -D warnings`, `cargo test --workspace` and `cargo gates`,
and reruns Dressrosa 11 and 28 with identical `.ass` and `.localized.ass` files and the same
`text_verify` verdicts.

1. **Decision and types.** A test program confirms how `redb` behaves when a second process
   opens a database (and whether a read-only open is possible while another process writes),
   and this plan's ownership rules are corrected to what it finds. Decision entry and the new
   law 6. Current `redb` and `rkyv` releases pinned in the workspace. `rkyv` derives on every `job_model` type, with string forms for
   `PathBuf` and ordered maps where `HashMap` does not archive; round-trip tests per type.
2. **Store, ownership and dump.** `JobStore` in `crates/pipeline/src/work_dir/` owns `job.redb`,
   opens it once per job, and exposes typed `put`/`get`/archived `view` per table. The runner
   passes worker outputs through `exchange/`. `tbd-subtitles dump <job_or_video> <table> [key]`
   prints JSON.
3. **Steps move, in graph order, straight to rkyv.** Each group moves reader and writer
   together, with its tests:
   - `probe_decode`, `shot_scan`, `separation`, `vad`;
   - `asr_parakeet`, `asr_whisper`, `diff_sheet`, `sound_events`;
   - `adjudicate`, `redecode_parakeet`, `redecode_whisper`, `readjudicate`, `sound_cues`;
   - `alignment`, `review` (with corrections and Fix It), `cues`, `qc`, `output`;
   - `text_detect`, `text_read`, `text_track`, `text_translate`, `text_review`;
   - `text_mask`, `text_inpaint`, `text_compose`, `text_verify`, `text_typeset`,
     `localized_video`.
   The window's readers (Overview, Check Lines, Check Text, report) move with the step they read.
4. **Resume and reruns in the store.** `step_records` replace `job.json` steps and
   `steps/*.worker.json`; `--rerun` becomes the one-transaction clear above; orphan cleanup on
   open.
5. **Per-frame tables.** `frames` and `readings` filled by `text_mask`, `text_verify` and
   following; the erase mask becomes per frame where the writing moves or its background
   changes. Measured on Dressrosa 11, 28 and a 60 fps video: time, RAM, VRAM, `job.redb` size.
6. **Library.** `library.redb`, matching by reading and crop hash, reuse measured over a batch
   of episodes.

## Boundaries

- Depends on: `crates/job_model` contracts, the step graph and runner in `crates/pipeline`, the
  worker processes of law 7.
- Used by: the runner, the GPU workers (through the runner), the CLI and the window.
- Rules: one owning process per database; files outside the database are synced before their
  record commits; every table has a layout version in the fingerprints; no JSON fallback;
  dialogue is never invented.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — steps and their order.
- [System overview](/documentation/architecture/system_overview.md) — processes and workers.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  visual steps that per-frame tables serve.
- [Work directory](/crates/pipeline/src/work_dir/README.md) — the files this replaces.
