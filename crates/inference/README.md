# Inference backends

The `inference` crate: the backends that run the models the GPU stages need, which are ONNX
Runtime, ggml and candle models and the language-model backends, and the model store that
downloads and verifies model files. It declares one module folder per backend; their code is not
written yet.

## Contents

```text
crates/inference/
├── Cargo.toml  the `inference` library package; depends on `child_process` and `job_model`
└── src/        one module folder per runtime, the language-model backends and the model store
```

## How it works

The crate is for the stages that run in a [worker process](/documentation/glossary.md#worker-process):
`stages` calls it from inside that process, which loads each model once and exits when the stage
is done. The modules divide the work by runtime: `onnx` for ONNX Runtime on CUDA, `ggml` for the
ggml-based crates, `candle` for the pure-Rust engine, `llm` for the language models that
adjudicate the [diff sheet](/documentation/glossary.md#diff-sheet), and `model_store` for the
models folder with its pinned manifest. Each module holds only its header, and the manifest links
no runtime crate yet. `src/README.md` describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p inference   # the module declarations; no runtime is linked
cargo test -p inference    # runs 0 tests: no module holds code yet
```

## Configuration

None: the crate reads no setting.

## Public surface

- The library `inference`, with the public modules `candle`, `ggml`, `llm` (holding
  `llm::claude_cli` and `llm::mistral_rs`), `model_store` and `onnx`; they hold no items yet.
- No binary.

## Boundaries

- Depends on: `child_process` and `job_model`, declared in `Cargo.toml` and not called yet.
- Used by: `crates/stages/`, which declares it as a dependency.
- Rules:
  - the crate sits in layer 1 and depends only on layer 0 crates (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - models are downloaded already exported, from pinned URLs with checksums, and never converted,
    and no two crates that bundle ggml link into one binary (the crate header in
    `crates/inference/src/lib.rs`; no gate holds these).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#recommended-stack) — the crates and
  model files for each capability.
- [Decisions](/documentation/decisions.md) — native runtimes where no pure-Rust engine competes,
  and one worker process per GPU stage.
- [System overview](/documentation/architecture/system_overview.md#models) — the models folder and
  its manifest.
