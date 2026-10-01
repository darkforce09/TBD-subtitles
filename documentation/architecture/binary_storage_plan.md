**Status:** live

# Binary storage: redb and rkyv

The approved move of every job's step outputs from loose JSON files into one embedded `redb`
database per job, with values archived by `rkyv`, and of approved signs into one library shared
by every episode. The owner approved it, and all six phases are built: the decision, the pinned
releases and the archived types; the worker channel, the job store, its ownership and `dump`;
every step's documents in the store; resume and reruns in the store; the per-frame tables; and
the library shared by episodes.

## Why

A JSON file per step, written through a part file and renamed and skipped while it is valid,
holds one document that is parsed whole. The next quality steps need data that grows with the
video:

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
  for the whole job, and writes its process id to `job.lock` once the open succeeds (written
  before, a refused open would overwrite the real owner's id). That open also repairs a file a
  crashed run left. Every caller in one process (the window's queue, its job threads, Fix It, the
  background `shot_scan`) gets the same handle from a registry of open jobs.
- **Workers never open it.** Almost every step runs in a worker process (all but `vad`,
  `diff_sheet`, `cues`, `text_review`, `qc` and `output`), and a background `shot_scan` runs beside
  other steps, so no worker could hold the file. A worker takes its inputs and returns its outputs
  over the [worker channel](#worker-channel): framed `rkyv` bytes on pipes, never files.
- **The window reads through its runner.** The window runs jobs in its own process and shares
  that process's one handle per job (a second handle in the same process fails like one
  from another process). When another process owns a job (a `tbd-subtitles process` run from a
  terminal), its open fails with `DatabaseAlreadyOpen`; the window shows the job as busy, reads
  `job.lock` to name the owner, retries after that process exits, and writes corrections only
  then; the window's line and text corrections and Fix It's writes follow the same rule.
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
  codec live in the layer-0 crate [`worker_channel`](/crates/worker_channel/README.md) with
  round-trip tests, used by the runner and all three worker binaries.
- **Inputs down stdin.** The runner reads each archived value a step reads from `job.redb` in one
  read transaction, writes its bytes as `Input` frames to the worker's stdin on a thread of its
  own (so a worker that reports progress before it has read every input never deadlocks), and
  closes stdin. The worker reads each frame into one buffer and uses it in place with
  `rkyv::access`; `unaligned` makes any buffer valid.
- **Per-frame rows down stdin.** A step that reads a per-frame table (`graph::reads_rows`:
  `text_compose`, `text_verify` and `localized_video` read `frames`) receives, after its
  documents, every row of that table in key order from the same snapshot, one `Input` frame per
  row read in place from its page. The worker reads its documents up to the first row
  (`read_documents`) and leaves the rows on the pipe; the task walks them once, one buffer at a
  time (`StepIo::frame_rows`), and keeps only what it folds them into (runs of equal shift per
  plate). The pipe's capacity holds the runner's sending thread back until the worker reads on,
  so neither side ever holds a table whole, and a worker drains whatever it did not read before
  `Done`, so the sending thread always ends cleanly. In the runner the same call walks the step's
  snapshot.
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

What redb holds in memory for a large uncommitted transaction follows its page cache, not the
transaction: 432,000 rows of 2 KB (a two-hour 60 fps per-frame table) peaked at 154 MiB with a
256 MiB cache and 557 MiB with redb's default 1 GiB, and the commit took 17–25 s
([measurement](/documentation/research/redb_large_transaction_memory.md)). One transaction per
step therefore fits the 8 GB limit at any video length; every job database opens with a 256 MiB
cache.

## Storage layout

```text
work/<job_id>/
├── job.redb      every step's output, the job record, step records and per-frame tables
├── job.lock      the process that owns job.redb
├── audio/        16 kHz PCM streams (large files, referenced by records)
├── visual/       crops, keyframe stills, masks, plates and patches (referenced by records), and
│                 the reading and translation caches
├── fix/calls/    Fix It's answered model calls, kept until a run finishes
├── logs/         step logs
├── sheet.txt     the diff sheet for reading
└── report.md     the human report

library.redb      beside work/: approved signs shared by every episode
```

A file outside the database is written and synced before the record that names it commits. When
a job is opened, the files in the step-owned folders (`audio/`,
`visual/{crops,keyframes,masks,plates,patches}`) that no record names are removed; the caches,
`fix/calls/`, `logs/`, `sheet.txt` and `report.md` stay.

## Tables in `job.redb`

| Table | Key | Value |
|---|---|---|
| `meta` | `"job_record"`, `"layout"` | the job record (video, settings, models); the layout version of every table |
| `step_records` | step name (`probe_decode` … `localized_video`) | fingerprint of its inputs, finished time, measures (with the worker's `Measure` frame) |
| `outputs` | step name (`asr_parakeet`, `redecode_whisper` are steps of their own), or `<step>/<part>` for a step's further document (`cues/dropped_sounds`, `text_typeset/ass`) | the step's output document |
| `corrections` | `"lines"`, `"text"`, `"fix"` | the owner's line and on-screen text corrections, and Fix It's record of its runs |
| `frames` | (occurrence id, frame number) | a `FrameRecord` written by `text_mask`: quad, follow score, shift and scale, erase mask as run-length rows relative to its plate, plate index |
| `readings` | (occurrence id, frame number) | a `VerifyReading` written by `text_verify`: Japanese found, English read, similarity, verdict |

Values are `rkyv` archives of the `job_model` types. Each per-frame table belongs to the one step
that writes it (`graph::writes_rows`); `dump <job> frames` prints every row as JSON Lines and
`dump <job> frames <occurrence>/<frame>` one.

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
`crates/pipeline/src/graph/`), plus the `frames` rows when `text_mask` is among them and the
`readings` rows when `text_verify` is, in one transaction; files they named become unreferenced
and are removed on the next open. Before a step runs, its record and the rows of the per-frame
table it owns go in a transaction of their own, so a step that runs again for a changed input
never leaves rows of frames its earlier run had beside its new ones. The
[decision entry](/documentation/decisions/storage.md) records the law.

## Library shared by episodes

`library.redb` in the app's data folder (`~/.local/share/tbd-subtitles/`, beside the default
`work/`; one per user, whichever work folder a job uses) holds approved signs in one table,
`signs`, keyed by the Japanese in Unicode NFKC without whitespace and a 64-bit difference hash of
the sign's keyframe crop (shrunk to 9 by 8 grey pixels); a `meta` row holds its layout version.
Each value is an rkyv `LibrarySign`: the English, its confidence, the lettering style, the patch
and mask of the approved replacement's first plate, and the jobs that recorded it, the first being
its origin. A sign is approved when `text_verify` kept its replacement baked with every reading
passed and no owner correction kept it in Japanese; the `output` step records it. A later
occurrence of another job with the same normalised Japanese and a crop hash at most 6 bits away
starts from the stored translation (`text_translate`, provenance `library`, no Claude call for a
keyframe of known signs alone) and lettering style (`text_compose`), and is still erased, composed
and read back in its own frames; a digest of the matched signs is part of both steps'
fingerprints, so a library change reruns only the jobs it touches, and a job never matches its own
signs. A Check Text correction that changes or removes a sign's English or keeps the Japanese, and
a retry, removes it. The window's Settings shows the library's size and clears it. A job run from
a terminal and the window can run at the same time, and a second open of the file fails at once,
so no process keeps `library.redb` open: a process opens it read-write for one transaction (the
lookups of one step, or one job's approved signs), closes it, and retries every 50 ms for up to
5 s while another handle holds it. The workers of the two steps that read it open it themselves,
named in `TBD_SUBTITLES_LIBRARY` by the runner, and hash every crop before they open it
([sign library](/crates/pipeline/src/library/)).

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
2. **Channel, store, ownership and dump (done).** The [worker channel](#worker-channel)'s frames
   and codec (`crates/worker_channel`), the worker's stdout moved aside, and the runner reading
   frames as bytes, carrying `Progress`, `ModelCall`, `Measure`, `Failed` and `Done`;
   `steps/*.worker.json` is gone while outputs are still JSON files. `JobStore` in
   `crates/pipeline/src/work_dir/store/` owns `job.redb` (redb 4.3.0, default features, 256 MiB
   cache), opened once per job per process through a registry, `job.lock` written after the open;
   the six tables with their layout versions in `meta`, typed `put`/`get`/archived `view`, record
   kinds that check an archive and print it as JSON, and the `Input`/`Output` path through
   `insert_reserve`, tested end to end on pipes. The runner and Fix It hold the store instead of
   the old pid lock, and the window shows a job another process owns as busy and starts it once
   that process ends. The redb memory of a large uncommitted transaction is measured (above).
   `tbd-subtitles dump <job_or_video> <table> [key]` prints JSON. Dressrosa 11 and 28, rerun from
   `text_mask` (the steps before it call Claude, whose answers vary), gave identical `.ass` and
   `.localized.ass` files and the same `text_verify` verdicts.
3. **Steps move, in graph order, straight to rkyv (done).** Every step reads and writes its
   documents only through the job store, through `tasks::StepIo`
   (`crates/pipeline/src/tasks/io.rs`): in the runner from a snapshot of the store and into the
   step's pending write, in a worker from the `Input` frames `graph::reads` names and as `Output`
   frames. `work_dir::store::keys` names every row: `outputs/<step>` for each step's document,
   `outputs/cues/dropped_sounds` and `outputs/text_typeset/ass` for the two further ones, and
   `corrections/lines`, `corrections/text` and `corrections/fix` for the owner's corrections and
   Fix It's record. The window's readers (queue, Overview, Check Lines, Check Text, the report)
   and `tools/visual_validation` read the same rows. What stays a file: the audio streams, the
   crops, keyframe stills, masks, plates, patches and previews the rows name, `sheet.txt`,
   `report.md`, the logs, and the caches (`visual/readings/`, `visual/translations/`,
   `claude-*.json`, `fix/calls/`). Jobs from before the store are not imported; they run again.
4. **Resume and reruns in the store (done).** The job record is `meta/job_record`, and no file
   outside the database holds it; `step_records` hold every step's record with its measure,
   committed with its outputs for in-process and worker steps alike; the fingerprints cover the
   stored table layouts and the stored corrections; `resume::is_valid` needs the step's
   documents and the files its rows name (`work_dir::store::files`); `--rerun` clears a step and
   every step that reads it in one transaction (`runner::rerun`); a file outside the database is
   synced before the record that names it commits, and an open removes the files in
   `OWNED_FOLDERS` that no row names. The window and Fix It write corrections through the owning
   process's store, one write transaction per change.
   A rerun clears `localized_video` to a record that names only the video it wrote, as its earlier
   one, so the rerun may replace that file. Dressrosa 11 and 28, their earlier documents stored
   once by a throwaway seeder outside the repository and rerun from `text_mask`, gave identical
   `.ass` and `.localized.ass` files and the same `text_verify` verdicts.
5. **Per-frame tables (done).** `text_mask` sends one
   `FrameRecord` row per frame of every occurrence that keeps its plates, each as one `Output`
   frame: the keyframe quad carried to the frame, the correlation of the keyframe writing there
   (the tracker's match for moving writing, the keyframe window against the frame's for still
   writing), the shift and scale, the frame's erase mask as run-length rows relative to its plate
   (`RleRun`), and the plate. The per-frame mask is the keyframe mask carried to the frame's
   placement, so still writing keeps exactly the masks, plates and files it had. A plate run now
   splits where a frame's mask overlaps the run's union mask by less than 0.85 intersection over
   union, where the scale changes or where the background changes; a plate is the union of its
   frames' rectangles and masks, so a one-pixel jitter shares a plate while writing that travels
   starts new ones. `text_compose` reads the rows and letters one patch per shift a plate's
   frames take (`Plate::shifted`); `localized_video` and `text_verify` blend each frame the patch
   of its own shift. `text_verify` writes each reading as a `VerifyReading` row and keeps only a
   verdict per occurrence (`TextCheck { samples, passed }`); Check Text reads the rows through the
   store. The rows reach the workers as described under
   [per-frame rows down stdin](#worker-channel). The `outputs`, `frames` and `readings` layouts
   are bumped (every table's layout is in every step's fingerprint, so an existing job reruns
   whole), and `text_mask`, `text_compose`, `text_verify` and `localized_video` carry new
   revisions. Dressrosa 11 and 28, rerun from `text_mask`, gave identical `.ass` and
   `.localized.ass` files and the same `text_verify` verdicts; the tables hold at most 1,898 frame
   rows per episode, `job.redb` stays under 20 MB, and a 60 fps copy of Dressrosa 11 adds time
   only to detection and the localized video's encode
   ([measurements](/documentation/research/per_frame_tables.md)).
6. **Library (done).** `crates/pipeline/src/library/` owns
   `library.redb` beside the default work folder (above), `job_model::onscreen::LibrarySign` its
   values; `text_translate` and `text_compose` read it, `output` records approved signs, Check
   Text removes rejected ones, and Settings shows its size and clears it. With an empty library no
   fingerprint, document or file changes, so Dressrosa 11 and 28 keep their outputs. Reuse over a
   batch of episodes: over Dressrosa 11–15 the library recorded 23 approved signs and none
   recurred, so no episode reused one ([measurements](/documentation/research/sign_library_reuse.md)).

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
- [Work directory](/crates/pipeline/src/work_dir/README.md) — the paths of the files that stay
  outside the database.
