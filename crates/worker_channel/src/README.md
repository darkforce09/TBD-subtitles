# Worker channel source

The `worker_channel` library: the frame codec, the addresses of job database values, the progress
payload, and the worker's side of the channel with its private frame descriptor.

## Contents

```text
crates/worker_channel/src/
├── address.rs   `Table`, `Key` and `Address`: the table and key that open an input or output payload
├── frame.rs     `Tag`, `Header`, `Frame`: write a frame from parts, read a header, a payload, a frame
├── lib.rs       the crate root: the module list and the crate header
├── progress.rs  `Progress`: the done and total counts of a progress frame, 16 bytes
├── tests/       unit tests over in-process pipes for every module
└── worker.rs    `install`, the send helpers, `read_inputs` and the `FrameSink` they share
```

## How it works

- `frame.rs` writes a header (the tag byte and the little-endian `u32` sum of the parts' lengths),
  then each part, then flushes; a total over `u32::MAX` is refused before anything is written.
  `read_header` loops over short reads, answers `Ok(None)` only when the stream ends before its
  first byte, `UnexpectedEof` when it ends inside the five header bytes, and `InvalidData` for a
  tag byte outside 1 to 7. `read_payload` grows its buffer as bytes arrive, so a false length never
  claims memory the stream does not deliver, and answers `UnexpectedEof` when the stream ends
  short.
- `address.rs` encodes an address as the table id (1 `meta` to 6 `readings`), the key kind (0 a
  name, 1 a frame of an occurrence), the key text's `u16` little-endian length and UTF-8 bytes,
  and for a frame key the frame number as a `u64` little-endian. `Address::read` reads it from the
  front of a payload and says how many bytes it took, so the archive is the rest of the payload.
- `progress.rs` is two little-endian `u64`s; any other length is `InvalidData`.
- `worker.rs` holds the process-wide `FrameSink<File>` in a `OnceLock`. `install` takes the
  stdout lock, flushes it, duplicates descriptor 1 with `try_clone_to_owned` (close-on-exec) and
  then `dup2`s stderr over descriptor 1 through `rustix::stdio::dup2_stdout`; a second call finds
  the sink set and does nothing. Each helper builds its payload and sends it through the sink's
  mutex (a lock poisoned by a panicked thread is taken over, never passed on as a panic); a helper
  answers `false` when the sink is unset or the write fails. `read_inputs` reads frames until a
  clean end and refuses any tag but `Input`.

## Public surface

- `frame::{Tag, Header, Frame, HEADER_LEN, write_frame, read_header, read_payload, read_frame}`:
  the runner's reader in `crates/pipeline/src/workers/frames.rs`, its inputs in
  `crates/pipeline/src/workers/channel/`, and their tests.
- `address::{Table, Key, Address}`: the addresses of `Input` and `Output` values, and the tables
  the `dump` subcommand names.
- `progress::{Progress, ENCODED_LEN}`: the `Progress` payload, decoded by the runner.
- `worker::{install, send, progress, model_call, output, measure, failed, done, read_inputs}`:
  `pipeline::tasks::worker_main` and the model-call layers of `apps/tbd_subtitles/` and
  `apps/tbd_subtitles_llm/`.

## Boundaries

- Depends on: `std` (`std::io`, `std::os::fd`, `std::sync`) and `rustix::stdio::dup2_stdout`.
- Used by: `crates/pipeline/src/tasks/mod.rs`, `crates/pipeline/src/workers/frames.rs`,
  `crates/pipeline/src/workers/channel/`, `crates/pipeline/src/work_dir/store/`,
  `apps/tbd_subtitles/src/cli/dump_command.rs`,
  `apps/tbd_subtitles/src/core/log_buffer/worker_channel.rs` and
  `apps/tbd_subtitles_llm/src/logging.rs`.
- Rules:
  - a frame is a tag byte, a little-endian `u32` length and the payload
    (`a_header_is_the_tag_then_the_length_little_endian` in `tests/frame.rs`), and a frame may
    carry no payload (`a_frame_may_carry_no_payload`);
  - both key kinds round trip and report their length, and a bad table, kind or key is
    `InvalidData` (`a_named_key_round_trips_and_says_how_long_it_was`,
    `a_frame_key_round_trips_with_its_frame_number`, `a_bad_table_kind_or_key_is_invalid_data` in
    `tests/address.rs`);
  - `read_inputs` reads every input until the stream ends and refuses any other frame
    (`inputs_are_read_until_the_stream_ends`,
    `any_other_frame_among_the_inputs_is_invalid_data` in `tests/worker.rs`);
  - no test calls `install`, which would rewire the test process's own stdout.

## Related documentation

- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#worker-channel) — the
  frames and the pipes they cross.
