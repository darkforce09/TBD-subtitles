# Worker channel data path

The runner's side of the step data a worker exchanges with it: the stored values a step reads,
sent down the worker's stdin, and the outputs it writes, read from its frames straight into the
job database and committed with the step's record.

## Contents

```text
crates/pipeline/src/workers/channel/
├── inputs.rs   `send_inputs`: a step's stored values as `Input` frames down the worker's stdin
├── mod.rs      the module header and `StepWrite` re-exported
├── outputs.rs  `StepWrite`: a step's outputs kept in one write transaction and committed with its record
└── tests/      end-to-end tests on real pipes, a thread playing the worker
```

## How it works

```text
job.redb ──read snapshot──▶ send_inputs (own thread) ──Input frames──▶ worker stdin ──▶ read_inputs
worker frame fd ──Output frames──▶ frames::read_frames ──▶ StepWrite::receive ──▶ redb reserved row
                                                        runner: StepWrite::commit(step, record)
```

`send_inputs` runs on a thread of its own, so a worker that reports progress before it reads every
input never deadlocks its runner. It opens one read snapshot of the job's `JobStore`, writes each
value as an `Input` frame (the value's `worker_channel::address` then its archive, straight from
the database's page) and closes the pipe; a value missing from the database ends it with an error.
`workers::run_worker` joins it after the worker's frames are read, and a failed send fails only a
step that otherwise finished, since a worker that ends early closes the pipe on it.

`StepWrite` begins the database's one write transaction on the first `Output` frame. For each one
it looks up the row's record kind (`work_dir::store::kinds`) before anything reaches the store: a
table a worker never writes (`meta`, `step_records`, `corrections`) or a key with no record kind
is a protocol error. It then calls `StoreWrite::reserve` for the archive's length and reads the
archive from the pipe into the reserved slice with no buffer between, and checks it in place with
the kind's bytecheck; a short read or a failed check leaves no row and is a protocol error, and
the caller stops the worker. `read_frames` hands it the rest of the frame as a bounded reader and
refuses a `StepWrite` that leaves any of it unread. `commit` puts the step's `StepRecord` in
`step_records` under the step's name and commits every row at once; a `StepWrite` dropped without
`commit` (a protocol break, a `Failed` frame, a missing `Measure` or `Done`, a non-zero exit, a
cancelled run) stores nothing. While a step's transaction is open, another step's first output
waits for it: the runner commits a step's outputs as soon as its record is built, the background
shot scan on its own thread.

No step declares inputs or sends outputs yet: every step still writes its JSON files, and the
runner passes no inputs.

## Boundaries

- Depends on: `crate::work_dir::{JobStore, store}` (`StoreRead::with_bytes`, `StoreWrite`,
  `kinds`), `worker_channel` (`address::Address`, `frame`), `job_model` (`StepName`, `StepRecord`).
- Used by: `crate::workers::run_worker` and `crate::workers::frames::read_frames`;
  `crate::runner::run_job` and `tools/visual_validation/src/pilot.rs`, which commit a finished
  step's `StepWrite`.
- Rules:
  - the inputs a runner sends reach the worker byte-exact, and a missing one is an error that
    closes the pipe (`the_runner_sends_stored_inputs_that_the_worker_reads_back_byte_exact`,
    `a_missing_input_is_an_error_and_closes_the_pipe` in `tests/channel.rs`);
  - outputs arrive in place and commit with the step's record, and a progress frame between them
    still reaches the progress sink (`outputs_arrive_in_place_and_commit_with_the_step_record`);
  - a `Failed` frame, a missing `Done`, a non-zero exit, an archive that does not check, an
    unknown output key, a table a worker never writes, a frame shorter than its address and a
    stream cut inside an output store nothing (`a_step_that_does_not_finish_stores_nothing`);
  - an unknown output is refused before the write transaction begins, and a dropped `StepWrite`
    stores nothing (`an_unknown_output_is_refused_before_the_step_write_begins`,
    `a_step_write_dropped_without_commit_stores_nothing`).

## Related documentation

- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#worker-channel) — the
  worker channel's frames, inputs down stdin and outputs straight into redb.
