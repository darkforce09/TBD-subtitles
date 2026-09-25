# Stack spike

The `stack_spike` binary (`stack-spike`): the measuring harness that proves each piece of the ML
stack on one real video before the pipeline is built around it. It downloads the pinned model
files and the CUDA 13 runtime, and is used by a developer on the host.

## Contents

```text
tools/stack_spike/
├── Cargo.toml  the `stack_spike` binary package and the reason for each dependency
└── src/        the command line and one module per command
```

## How it works

`stack-spike fetch` walks the model store's manifest: each model whose files are not all in
place is downloaded file by file, and each CUDA runtime archive not yet unpacked is downloaded,
checked and unpacked into the runtime folder. `--dry-run` lists every entry with its size and
state and downloads nothing; `--only <id>` limits the run to named models or archives.

```text
stack-spike fetch ──▶ inference::model_store ──▶ models/<model>/ and runtime/<folder>/
```

## Getting started

Run these from the repository root:

```bash
cargo run -p stack_spike -- fetch --dry-run   # every pinned file with its size and state
cargo run --release -p stack_spike -- fetch   # download and check what is missing
```

The full fetch downloads about 14 GiB; every file is checked against its SHA-256 before it is
used.

## Configuration

- `XDG_DATA_HOME`, else `HOME`: where the models and runtime folders live (read by
  `crates/inference/src/model_store/mod.rs`).

## Public surface

- The binary `stack-spike` with the command `fetch [--only <id>]... [--dry-run]`.
- No library.

## Boundaries

- Depends on: `crates/inference/` (the model store); `clap` and `anyhow`.
- Used by: a developer measuring the stack; nothing depends on it.
- Rules:
  - the tool depends only on the workspace crates the tool table in
    `tools/repo_gates/src/layout.rs` lists for it (`cargo gates crate-layering`);
  - the video and its folder are only read (the crate header in `src/main.rs`).

## Related documentation

- [Roadmap](/documentation/roadmap.md) — the stack items the tool measures.
- [Rust ML stack](/documentation/research/rust_ml_stack.md) — the crates and model files under
  test.
- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — the CUDA runtime the tool fetches.
