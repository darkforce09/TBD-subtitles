# Job store

The one handle of a job's database, `job.redb`, that a process holds: who owns it, the six tables
and the layout version of each, the name of every row, rkyv rows written in one transaction and
read back typed, in place or as bytes, the record kind of every row, and the files outside the
database that rows name.

## Contents

```text
crates/pipeline/src/work_dir/store/
├── files.rs     the files each step's rows name, whether they are there, and the orphan cleanup
├── job_rows.rs  the job record and step records typed, the `put_*` fixtures, `read_job`
├── keys.rs      the name of every row: `output_key`, `output_parts`, `record_key`, the constants
├── kinds.rs     `RecordKind` and `kind`: the type each table and key archives, checked and as JSON
├── mod.rs       `JobStore`: the open, the registry, `job.lock`, the layout check and transactions
├── records.rs   `StoreWrite` (put, reserve, get, remove, clear, commit), `StoreRead` (get, view, …)
├── tables.rs    one redb definition per table and `LAYOUT_VERSIONS`
└── tests/       unit tests of the store and `scratch.rs`, the crate's test job database
```

## How it works

`JobStore::open` creates the job folder and opens `job.redb` read-write with a 256 MiB page cache
([measurement](/documentation/research/redb_large_transaction_memory.md)); a file a killed writer
left is repaired on that open, keeping every commit, and the repair is logged. Opens go through a
process-wide registry of weak handles keyed by the job folder, so a second open in the same
process returns the same `Arc`, and the last one dropped closes the database. redb locks the file
for the life of the handle: a second handle, in this process or another, fails at once with
`DatabaseAlreadyOpen` ([measurements](/documentation/research/redb_multi_process.md)). Then the
open reads `job.lock`: when it names this process, the handle it sees is one of ours still
closing on another thread, and the open retries every 20 ms for up to 1 s; otherwise it returns
`PipelineError::busy` with the pid `job.lock` names, or none. Only after an open succeeds does the
store write this process's pid to `job.lock` (through a part file); a store that closes removes
`job.lock` while it still names this process, after the database is closed, and it holds the
registry meanwhile, so a new open in the process never sees the file half closed.
`JobStore::open_existing`, for reading a job, does the same with a file that must exist, and
creates nothing when it does not.

Every fresh open runs one transaction: it reads the `TableLayouts` under the `meta` key `layout`,
drops each table whose stored version differs from `LAYOUT_VERSIONS` (or has none while the table
exists), creates the six tables and stores the current versions. There is no migration: a
dropped table's steps run again. Then `files::remove_orphans` removes every file under `audio/`
and `visual/{crops,keyframes,masks,plates,patches}` that no stored row names (a failure is
logged, never fatal); caches (`visual/readings/`, `visual/translations/`, `claude-*.json`,
`fix/calls/`), `logs/`, `report.md` and `sheet.txt` are never touched.

| Table | Key | Definition |
|---|---|---|
| `meta` | a name (`layout`) | `NamedTable` |
| `step_records` | a step name | `NamedTable` |
| `outputs` | a step name, or an engine or pass | `NamedTable` |
| `corrections` | a name | `NamedTable` |
| `frames` | (occurrence id, frame number) | `FramedTable` |
| `readings` | (occurrence id, frame number) | `FramedTable` |

`JobStore::write` begins the database's one write transaction (a second waits for it) as a
`StoreWrite`: `put` archives a value with rkyv, `reserve` lets a caller fill the value's bytes in
place (such as straight from a worker's pipe) and leaves no row when the fill fails, `remove`
takes a row out, and `commit` makes them visible at once. `JobStore::read` gives a `StoreRead`
snapshot: `get` checks and copies a row out, `view` checks the archive and reads it in place,
`raw` copies the bytes and `keys` lists a table; `rows` walks a per-frame table in key order (all
of it, or one occurrence's range), handing each row's bytes in place one at a time, and `rows_as`
the same rows checked and copied out as a type. Each takes a `worker_channel::address` table and
key; a key whose kind is not its table's is an error. `StoreWrite::get` reads a row inside the
write, for a read-change-write, and `clear` empties a table.

`keys` names every row: a step's documents are `outputs/<step>`, and `outputs/<step>/<part>` for
the further ones `output_parts` lists (`cues/dropped_sounds`, `text_typeset/ass`); its record is
`step_records/<step>`; the job record is `meta/job_record` and the last run `meta/last_run`; the
owner's corrections are `corrections/lines` and `corrections/text`, and Fix It's record is
`corrections/fix`.
`job_rows` reads the job record, the last run and the step records typed
(`StoreRead::job_record`, `job_run`, `step_record`, `step_records`, `output`), puts one row per
committed transaction (`JobStore::put_job_run` for the runner; `put_output`, `put_job_record`,
`put_step_record` for the runner's fixtures and the tests of the crate and the app), and reads a
job from its folder for a caller that holds no store (`read_job`, `read_stored`).

`files::named_files` lists the files one step's rows name: the probe's mix, the separation's
stems, the crops and keyframe stills of every on-screen text document, the sources, masks, fills,
patches (a plate's further `shifted` ones too) and previews of every replacement document, and
the subtitle files and the localized video beside the video. A reference that is not a plain relative path, or a detection with an
occurrence that names no crop, is malformed. `resume::is_valid` and the orphan cleanup both read
it, so what a step must keep and what a cleanup keeps are one list.

`kinds::kind` names the type behind a table and key as a `RecordKind`, whose `check` runs rkyv's
bytecheck over an archive and whose `json` reads it back and turns it into JSON. The worker
channel checks every output a worker sends with it before the row is kept
(`crate::workers::channel`), and the app's `dump` subcommand prints rows with it.

| Table | Key | Record type |
|---|---|---|
| `meta` | `job_record`, `layout`, `last_run` | `JobRecord`, `TableLayouts`, `JobRun` |
| `step_records` | any step name | `StepRecord` |
| `corrections` | `lines`, `text`, `fix` | `Corrections`, `TextCorrections`, `FixRecord` |
| `outputs` | a step name | the document the step writes: `ProbeDecoded`, `ShotChanges`, `SpeechPlan`, `EngineTranscript` (both ASR steps), `Vec<Utterance>` (`diff_sheet`), `Vec<SoundEvent>`, `AdjudicationPass` (`adjudicate`, `readjudicate`), `Redecode` (both redecodes), `SoundCues`, `Aligned` (`alignment`, `review`), `CueTrack` (`cues`), `TextDocument` (`text_detect` … `text_review`, `text_typeset`), `ReplacementDocument` (`text_mask`, `text_inpaint`, `text_compose`), `VerifiedReplacements`, `QcReport`, `OutputRecord`, `LocalizedVideoRecord` |
| `outputs` | `<step>/<part>` | a step's further document: `cues/dropped_sounds` is `Vec<String>`, `text_typeset/ass` is `String` |
| `frames` | (occurrence, frame) | `FrameRecord` |
| `readings` | (occurrence, frame) | `VerifyReading` |

`separation` writes no document (its stems are files), so its key has no kind, nor has any other
key the table does not list. `kinds::shown` writes a key as the owner types it: a name, or
`<occurrence>/<frame>` for a per-frame key.

## Boundaries

- Depends on: `redb` 4.3.0, `rkyv` (the format `job_model` pins), `serde_json` (a kind's JSON),
  the `job_model` types (`store::TableLayouts` among them) and `subtitle_formats::cue::CueTrack`
  the kinds name, `worker_channel::address::{Table, Key}`, `tracing`, and
  `super::{WorkDir, write_text}`.
- Used by: `crate::runner::run_job` and `crate::fix_it::fix_job`, which hold a `JobStore` while
  they run; `crate::workers` (the worker channel's inputs, outputs and their kinds);
  `crate::tasks::StepIo` and `crate::graph` (the keys); `crate::resume` (the rows and
  `named_files`); `crate::runner::rerun` and `crate::report` (the rows they clear and read);
  `tools/visual_validation/src/pilot.rs` (`JobStore`) and
  `tools/visual_validation/src/job_rows.rs` (`read_stored`); the app's `dump` subcommand (`kind`),
  and its queue (`job_queue/services/video_files`, `queue_store`, `time_left`), report, line
  review and text review (`read_job`, `read_stored`, `JobStore`, `StoreRead`).
- Rules:
  - an open creates the six tables, the layout and `job.lock`
    (`an_open_creates_the_six_tables_the_layout_and_the_lock` in `tests/store.rs`);
  - every caller in a process shares one handle, and the last one closes it and removes
    `job.lock` (`every_caller_in_a_process_shares_one_handle_and_the_last_one_closes_it`);
  - a second handle is refused, and a foreign owner is busy with its pid or none, its `job.lock`
    untouched (`a_second_handle_is_refused_and_a_foreign_owner_is_busy`);
  - a changed layout version drops that table only, and a table without a stored version is
    dropped (`a_changed_layout_version_drops_that_table_and_keeps_the_others`,
    `a_table_without_a_stored_version_is_dropped`);
  - `open_existing` never creates a database or a folder
    (`opening_an_existing_store_never_creates_one`);
  - rows come back by `get` and `view`, frame rows in frame order, and a key of the wrong kind is
    an error (`a_step_record_comes_back_by_get_and_by_view`,
    `a_frame_row_comes_back_and_lists_in_frame_order`, `a_key_of_the_wrong_kind_is_an_error` in
    `tests/records.rs`);
  - a reserved row is filled in place and a failed fill leaves none, and nothing is visible before
    `commit` (`a_reserved_row_is_filled_in_place_and_a_failed_fill_leaves_none`,
    `a_removed_row_is_gone_and_an_uncommitted_write_changes_nothing`);
  - every document a task or the owner writes has a record kind whose check accepts its
    archive and refuses garbage, every step but `separation` has one, and a key that names no
    type is an error
    (`every_document_a_task_writes_has_a_kind_that_checks_it_and_refuses_garbage`,
    `every_step_but_separation_has_an_output_document`, `unknown_keys_are_errors` in
    `tests/kinds.rs`);
  - every output key of every step has a kind and a sample
    (`every_output_key_of_every_step_has_a_kind_and_a_sample`), and every listed name parses
    back to its step and part (`every_listed_output_name_parses_back_and_nothing_else_does` in
    `tests/keys.rs`);
  - the job record and step records read back as they were put, and a reader without a store
    creates nothing (`tests/job_rows.rs`);
  - a missing or malformed file a record names makes it unresumable, and an open removes exactly
    the owned files no row names
    (`opening_a_job_removes_the_owned_files_no_row_names_and_nothing_else`,
    `a_stored_mask_document_keeps_its_masks_and_an_unnamed_mask_is_removed_on_open` and
    `the_checked_replacements_name_their_files_and_the_localized_video_its_file` in
    `tests/files.rs`);
  - whoever changes a type stored in a table bumps that table's version in `LAYOUT_VERSIONS`,
    and whoever adds a document a step writes lists it in `keys::output_parts` and adds its kind
    (review).

## Related documentation

- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#process-ownership) —
  who owns `job.redb`, its tables and why a layout version drops a table.
- [redb across processes](/documentation/research/redb_multi_process.md) — the measured locking
  and repair behaviour the ownership rules follow.
