# TBD Subtitles ggml worker

The `tbd_subtitles_ggml` crate, which builds the `tbd-subtitles-ggml` binary: the
[worker process](/documentation/glossary.md#worker-process) that runs the two Whisper steps of a
job through CrispASR (ggml with CUDA). It is a binary of its own because ggml and ONNX Runtime
corrupt each other's heap in one process; the job runner in `tbd-subtitles` starts it, and people
never run it by hand.

## Contents

```text
apps/tbd_subtitles_ggml/
├── build.rs    with `crispasr`, the rpath to the libcrispasr and ggml libraries crispasr-sys built
├── Cargo.toml  the binary package, its `crispasr` feature and the reason for each dependency
└── src/        the `worker` command line and its tests
```

## How it works

The job runner in `crates/pipeline/` finds this binary beside its own
(`Binaries::beside_current_exe` in `crates/pipeline/src/workers/mod.rs`) and starts it as
`tbd-subtitles-ggml worker <step> <job dir>`, with the CUDA runtime's `LD_LIBRARY_PATH` added.
`src/main.rs` parses the step, accepting only the steps `pipeline::graph::placement` gives to
`Binary::Ggml` (`asr_whisper`, the second speech engine, and `redecode_whisper`, its re-decode of
unsure utterances), then hands it to `pipeline::tasks::worker_main`, which loads the job, runs
the step, writes its output and its measure file into the job's
[work directory](/documentation/glossary.md#work-directory), and prints `progress <done> <total>`
lines on stdout for the runner to forward.

```text
tbd-subtitles (job runner) ──▶ tbd-subtitles-ggml worker <step> <job dir>
                                   └─▶ pipeline::tasks::worker_main(step, job dir, Binary::Ggml)
                                          └─▶ Whisper through CrispASR, over the job's chunk plan
```

The step code lives in the product crates; this crate holds only the command line. A build
without the `crispasr` feature parses the same arguments and refuses every step with the reason.

## Getting started

Run these from the repository root. The feature build needs cmake and the CUDA 13.4 toolkit from
the runtime folder, takes a few minutes the first time, and must land in the same folder as
`tbd-subtitles`:

```bash
env PATH="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4/bin:$PATH" CUDACXX="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4/bin/nvcc" CUDAToolkit_ROOT="$HOME/.local/share/tbd-subtitles/runtime/cuda-13.4" CUDAARCHS=86 cargo build --release -p tbd_subtitles_ggml --features crispasr
cargo build --release -p tbd_subtitles   # the main binary, beside it in target/release/
cargo test -p tbd_subtitles_ggml         # the command-line tests; no GPU, no feature needed
```

## Configuration

- The Cargo feature `crispasr` (off by default) enables `pipeline/crispasr` and `crispasr-sys`,
  linking CrispASR and ggml with CUDA (`Cargo.toml`); `src/main.rs` refuses to run without it.
- `DEP_CRISPASR_LIBDIR` and `DEP_CRISPASR_GGMLDIR`, set by crispasr-sys during a feature build:
  `build.rs` writes them into the rpath.
- `LD_LIBRARY_PATH` must hold the CUDA 13 `lib/` folder at run time; the job runner sets it.

## Public surface

- The binary `tbd-subtitles-ggml`, for the job runner alone; it must sit in the same folder as
  `tbd-subtitles`. No library target.

## Commands

The binary has one command; `--help` prints its usage and `--version` the version.

### worker

- Synopsis: `tbd-subtitles-ggml worker <STEP> <JOB_DIR>`
- Does: runs `<STEP>`, which is `asr_whisper` or `redecode_whisper`, over the job in `<JOB_DIR>`
  in this process; any other step is refused as one that runs in `tbd-subtitles`.
- Exit codes: 0 the step finished; 1 with `tbd-subtitles-ggml:` and the error chain on stderr,
  when the build has no CrispASR or the step failed; 2 on a usage error, including a step that
  is not a Whisper step.
- Example: `target/release/tbd-subtitles-ggml worker asr_whisper ~/.local/share/tbd-subtitles/work/<job>`

## Boundaries

- Depends on: `crates/pipeline/` (`graph`, `tasks::worker_main`) and `crates/job_model/`
  (`StepName`); `crispasr-sys` for its build metadata only; `clap` and `anyhow`.
- Used by: the job runner in `crates/pipeline/`, started from `tbd-subtitles`; nothing links it.
- Rules:
  - the crate sits on the top product layer beside `tbd_subtitles` in the layer table of
    `tools/repo_gates/src/layout.rs`, and no crate depends on it (`cargo gates crate-layering`);
  - only the Whisper steps are accepted (`only_the_whisper_steps_are_accepted` in
    `src/tests/cli.rs`);
  - a build without `crispasr` refuses to run instead of pretending to
    (`a_build_without_crispasr_refuses_to_run`);
  - it never loads ONNX Runtime (the header in `src/main.rs`).

## Related documentation

- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why ggml runs in a binary apart from ONNX Runtime.
- [Whisper large-v3 through CrispASR is the second speech engine](/documentation/decisions/stack_and_pipeline.md#2026-09-26--whisper-large-v3-through-crispasr-is-the-second-speech-engine)
  — the engine this binary runs.
- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — the CUDA toolkit the feature build needs.
