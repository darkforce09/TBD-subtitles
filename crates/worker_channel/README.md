# Worker channel

The `worker_channel` crate: the framed binary messages a step's
[worker process](/documentation/glossary.md#worker-process) and the job runner exchange on pipes,
and the worker's side of that exchange. The job runner in `crates/pipeline/` reads the frames, and
all three app binaries send them.

## Contents

```text
crates/worker_channel/
├── Cargo.toml  the `worker_channel` library package; its one dependency is `rustix`, for `dup2`
└── src/        the frame codec, the database addresses, the progress payload and the worker side
```

## How it works

Every message is one frame: a tag byte, a little-endian `u32` payload length, and the payload.
The seven tags are `Input` and `Output` (an address, then one `rkyv` archive), `Progress` (two
little-endian `u64` counts), `ModelCall` (a model call's JSON), `Measure` (the worker's
`WorkerMeasure` as an `rkyv` archive), `Failed` (a UTF-8 message) and `Done` (empty). The codec
never decodes a payload as text, so a byte sequence that is not UTF-8 crosses unchanged, and a
reader tells a clean end before a header apart from a stream cut inside a frame.

```text
worker process                                       job runner (crates/pipeline/src/workers/)
  worker::install()   fd 1 ─▶ copy of fd 2 (the step log)
                      private close-on-exec copy of the stdout pipe
  worker::progress / model_call / output / measure / done / failed
        └─ FrameSink: one lock, header + parts, flush ──▶ pipe ──▶ frame::read_header, per tag
  worker::read_inputs ◀── stdin ◀── Input frames, from job.redb on a thread of the runner's
```

A worker calls `worker::install` first thing, before any native library loads: it keeps a
close-on-exec duplicate of its stdout pipe for frames and points descriptor 1 at stderr, so what
whisper.cpp, ONNX Runtime or mistral.rs print lands in the step log and never in the frame stream,
and FFmpeg or `claude` children never inherit the frame pipe. After that each helper sends one
frame under one lock, so frames from different threads never interleave. `worker::read_inputs`
reads the `Input` frames a runner writes to the worker's stdin. `src/README.md` describes each
file.

## Getting started

Run these from the repository root:

```bash
cargo build -p worker_channel   # the library alone
cargo test -p worker_channel    # 22 unit tests over in-process pipes; well under a second
```

The tests never call `worker::install`, which would rewire the test process's own stdout; they
drive the same send path through `FrameSink` over a `std::io::pipe`.

## Configuration

None: the crate reads no setting, file or feature.

## Public surface

- `frame`: `Tag`, `Header`, `Frame`, `HEADER_LEN`, `write_frame`, `read_header`, `read_payload`
  and `read_frame`, for the job runner in `crates/pipeline/src/workers/frames.rs` and
  `crates/pipeline/src/workers/channel/`, and their tests.
- `address`: `Table`, `Key` and `Address` with `encode` and `read`, the table and key an `Input`
  or `Output` value belongs to, which the runner's worker channel routes into the job database.
- `progress`: `Progress` with `encode` and `decode`, and `ENCODED_LEN`, the `Progress` payload,
  for `crates/pipeline/`.
- `worker`: `install`, `send`, `progress`, `model_call`, `output`, `measure`, `failed`, `done` and
  `read_inputs`, for `crates/pipeline/src/tasks/mod.rs` (`worker_main`) and the model-call layers
  of `apps/tbd_subtitles/src/core/log_buffer/worker_channel.rs` and
  `apps/tbd_subtitles_llm/src/logging.rs`.
- No binary.

## Boundaries

- Depends on: `std` and `rustix` 1 (`std`, `stdio`: `dup2_stdout`); no workspace crate. Linux
  only, through `std::os::fd`.
- Used by: `crates/pipeline/` (the runner's frame reader, its inputs and outputs, and
  `worker_main`), `apps/tbd_subtitles/src/cli/dump_command.rs` (`address::{Table, Key}`),
  `apps/tbd_subtitles/` and `apps/tbd_subtitles_llm/` (their model-call layers).
- Rules:
  - the crate sits in layer 0 and depends on no workspace crate (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - every tag crosses a pipe unchanged, and a payload that is not UTF-8 survives byte for byte
    (`every_tag_goes_through_a_pipe_unchanged`,
    `a_payload_that_is_not_text_survives_byte_for_byte` in `src/tests/frame.rs`);
  - an empty stream is a clean end, a stream cut inside a header or payload is `UnexpectedEof`,
    and an unknown tag is `InvalidData` (`an_empty_stream_is_a_clean_end`,
    `a_stream_cut_inside_a_header_is_an_unexpected_end`,
    `a_stream_cut_inside_a_payload_is_an_unexpected_end`, `an_unknown_tag_is_invalid_data`);
  - frames sent from many threads stay whole (`frames_from_many_threads_stay_whole` in
    `src/tests/worker.rs`), and a send to a closed pipe is an error, never a panic
    (`a_closed_pipe_is_an_error_not_a_panic`).

## Related documentation

- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#worker-channel) — the
  worker channel's design: inputs down stdin, outputs into the job database.
- [System overview](/documentation/architecture/system_overview.md) — the worker processes and
  the binaries that run them.
