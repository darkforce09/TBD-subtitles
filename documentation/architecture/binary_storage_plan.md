**Status:** live

# Binary storage: redb and rkyv

The approved move of every job's step outputs from loose JSON files into one embedded `redb`
database per job, with values archived by `rkyv`, and of approved signs into one library shared
by every episode. The owner approved it; phase 1 (the decision, the pinned releases and the
archived types) is built, and no step reads or writes a database yet.

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

`redb` 4.3.0 with default features (the pinned release) allows one process per database file while
it writes. A read-write open locks the file exclusively and a read-only open locks it shared, with
non-blocking open-file locks, so while a process holds `job.redb` read-write every other open, read
or write, from another process or from a second handle in the same process, fails at once with
`DatabaseAlreadyOpen`; read-only opens share only with each other. A file left by a killed writer
refuses read-only opens (`RepairAborted`) until a read-write open repairs it, which takes a few
milliseconds and keeps every committed transaction. The locks hold across the container and the
host. The `experimental-multiprocess` feature's `SingleWriter` mode would let readers open beside
the writer, but it is part of the redb 5 API preview, so this plan does not use it
([measurements](/documentation/research/redb_multi_process.md)). The pipeline runs GPU steps in
separate worker processes (law 7), and the window reads a job while it runs, so:

- **The runner owns `job.redb`.** Only the process running the job opens it, read-write, once,
  for the whole job, and writes its process id to `job.lock` first. That open also repairs a file
  a crashed run left.
- **Workers never open it.** Almost every step runs in a worker process (all but `vad`,
  `diff_sheet`, `cues`, `text_review`, `qc` and `output`), and a background `shot_scan` runs beside
  other steps, so no worker could hold the file. A worker takes its inputs and returns its outputs
  over the [worker channel](#worker-channel): framed `rkyv` bytes on pipes, never files.
- **The window reads through its runner.** The window runs jobs in its own process and shares
  that process's one handle per job (a second handle in the same process fails like one
  from another process). When another process owns a job (a `tbd-subtitles process` run from a
  terminal), its open fails with `DatabaseAlreadyOpen`; the window shows the job as busy, reads
  `job.lock` to name the owner, retries after that process exits, and writes corrections only
  then; `review.json.lock` and Fix It's writes move to the same rule.
- **A job no process runs** is opened read-write by whichever process needs it (the window, `dump`,
  a correction run), so a crashed job is repaired before anything reads it.
- **Fix It and corrections** write through the owning process in one transaction each.

## Worker channel

A worker and its runner exchange step data as framed binary on two pipes, so an output crosses
one pipe, is copied once into redb's pages and reaches the disk once, at commit. No input or
output passes through a file. Large media stay files: the 16 kHz audio streams, the stills, crops,
plates and patches, and the localized video, which FFmpeg and the image code read and which
records name by path.

- **Frames.** Every message is one frame: a one-byte tag, a little-endian `u32` length, and that
  many bytes. `Input` and `Output` frames carry a table, a key and one `rkyv` archive; `Progress`,
  `ModelCall` (the model call's JSON, as the log window shows it), `Measure` (the worker's
  `WorkerMeasure`), `Failed` (a message) and `Done` carry the rest. The frame types and their
  codec live in a layer-0 crate with round-trip tests, used by the runner and all three worker
  binaries.
- **Inputs down stdin.** The runner reads each archived value a step reads from `job.redb` in one
  read transaction, writes its bytes as `Input` frames to the worker's stdin on a thread of its
  own (so a worker that reports progress before it has read every input never deadlocks), and
  closes stdin. The worker reads each frame into one buffer and uses it in place with
  `rkyv::access`; `unaligned` makes any buffer valid.
- **Outputs on a private descriptor.** At start, before any native library loads, the worker
  duplicates its stdout to a new descriptor it keeps for frames and points descriptor 1 at stderr
  (`rustix`, safe calls), so anything whisper.cpp, ONNX Runtime or mistral.rs prints goes to the
  step log and never into the frame stream. The runner reads the frames from the child's stdout
  pipe as bytes, never as lines.
- **Straight into redb.** On the first `Output` frame the runner begins the step's write
  transaction; for each one it calls `insert_reserve(key, len)` and reads the frame's bytes from
  the pipe directly into the reserved slice, with no buffer between, then checks them with
  `rkyv::access` (bytecheck). Per-frame tables arrive as one frame per row, so memory stays
  bounded by a row, not a table.
- **Commit.** After `Done` and a zero exit status the runner writes the step record (the worker's
  `Measure`, its own wall time and peak memory, the fingerprint) in the same transaction and
  commits it. Anything else — a missing `Done`, a failed check, a non-zero exit, a `Failed` frame —
  drops the transaction, so nothing of the step is stored.
- **Cancellation.** The cancel flag and the step's deadline kill the worker's process group as
  today (`child_process`); the pipe ends early, the read fails and the uncommitted transaction is
  dropped. A worker whose runner dies gets `PR_SET_PDEATHSIG` and a closed pipe.
- **In-process steps** (`vad`, `diff_sheet`, `cues`, `text_review`, `qc`, `output`, and the
  on-screen steps when the runner runs them itself) read and write the store directly through the
  runner's handle, in one transaction per step.
- **One writer at a time.** redb has one write transaction per database; while one step's
  transaction is open, a second step's first `Output` waits for it (the background `shot_scan`
  beside a GPU step), and that worker waits on its full pipe. Neither step reads the other's
  output, so the wait always ends.

Phase 2 measures what redb holds in memory for a large uncommitted transaction (a `text_mask`
output for a two-hour 60 fps video) before per-frame tables rely on one transaction per step.

## Storage layout

```text
work/<job_id>/
├── job.redb      every step's output, the job record, step records and per-frame tables
├── job.lock      the process that owns job.redb
├── audio/        16 kHz PCM streams (large files, referenced by records)
├── visual/       keyframe stills, crops, plates and patches (referenced by records)
├── logs/         step logs
└── report.md     the human report

library.redb      beside work/: approved signs shared by every episode
```

A file outside the database is written and synced before the record that names it commits. When
a job is opened, files no record names are removed.

## Tables in `job.redb`

| Table | Key | Value |
|---|---|---|
| `meta` | `"job_record"`, `"layout"` | the job record (video, settings, models); the layout version of every table |
| `step_records` | step name (`probe_decode` … `localized_video`) | revision, fingerprint of its inputs, finished time, measures (today `job.json` steps and the worker's `Measure` frame) |
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

Law 6 reads: *each step commits its output and its record in one transaction, and is skipped
while its record's revision and input fingerprint are current.* A rerun of a step deletes that
step's output and record and those of every step that reads it (the graph in
`crates/pipeline/src/graph/`), plus their `frames` and `readings` rows, in one transaction; files
they named become unreferenced and are removed on the next open. The
[decision entry](/documentation/decisions/storage.md) records the law.

## Library shared by episodes

`library.redb` holds approved signs keyed by the normalised Japanese and a perceptual hash of the
sign's keyframe crop: the English, the lettering style, and the patch and mask of the approved
replacement. A later occurrence whose crop matches starts from the stored translation and
lettering and is still erased, composed and read back in its own frames. The window's Settings
shows the library's size and clears it. A job run from a terminal and the window can run at the
same time, and a second open of the file fails at once, so no process keeps `library.redb` open: a
process opens it read-write for one transaction (a lookup, or one approved sign), closes it, and
retries after a short wait while another process holds it.

## Phases

Each phase passes `cargo fmt`, `clippy -D warnings`, `cargo test --workspace` and `cargo gates`,
and reruns Dressrosa 11 and 28 with identical `.ass` and `.localized.ass` files and the same
`text_verify` verdicts.

1. **Decision and types (done).** `tools/redb_process_probe` measured how `redb` behaves when a
   second process opens a database ([findings](/documentation/research/redb_multi_process.md)),
   and the ownership rules above follow them. The [decision entry](/documentation/decisions/storage.md)
   and the new law 6. `redb` 4.3.0 pinned by the probe (the pipeline takes the same pin in phase
   2) and `rkyv` 0.8.18 by `job_model`, its format pinned to `unaligned`, `little_endian` and
   `pointer_width_32`. `rkyv` derives on every `job_model` type,
   `PathBuf` archived as a UTF-8 string, both maps already ordered; round-trip tests per type.
2. **Channel, store, ownership and dump.** First the [worker channel](#worker-channel)'s frames
   and codec, the worker's stdout moved aside, and the runner reading frames as bytes, carrying
   `Progress`, `ModelCall`, `Measure`, `Failed` and `Done` while outputs are still JSON files.
   Then `JobStore` in `crates/pipeline/src/work_dir/` owns `job.redb` (redb 4.3.0, default
   features), opens it once per job with `job.lock`, and exposes typed `put`/`get`/archived `view`
   per table and the `Input`/`Output` path through `insert_reserve`; the redb memory of a large
   uncommitted transaction is measured. `tbd-subtitles dump <job_or_video> <table> [key]` prints
   JSON.
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
4. **Resume and reruns in the store.** `step_records` replace `job.json` steps and hold the
   worker's `Measure`; `--rerun` becomes the one-transaction clear above; orphan cleanup on
   open.
5. **Per-frame tables.** `frames` and `readings` filled by `text_mask`, `text_verify` and
   following; the erase mask becomes per frame where the writing moves or its background
   changes. Measured on Dressrosa 11, 28 and a 60 fps video: time, RAM, VRAM, `job.redb` size.
6. **Library.** `library.redb`, matching by reading and crop hash, reuse measured over a batch
   of episodes.

## Boundaries

- Depends on: `crates/job_model` contracts, the step graph and runner in `crates/pipeline`, the
  worker processes of law 7.
- Used by: the runner, the worker processes (through the worker channel), the CLI and the
  window.
- Rules: one owning process per database; files outside the database are synced before their
  record commits; every table has a layout version in the fingerprints; no JSON fallback;
  dialogue is never invented.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — steps and their order.
- [System overview](/documentation/architecture/system_overview.md) — processes and workers.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  visual steps that per-frame tables serve.
- [Work directory](/crates/pipeline/src/work_dir/README.md) — the files this replaces.
