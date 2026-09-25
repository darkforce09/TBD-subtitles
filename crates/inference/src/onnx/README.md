# ONNX Runtime models

Models run through ONNX Runtime (the `ort` crate) on CUDA: vocal separation now, and speech
recognition, [CTC](/documentation/glossary.md#ctc) alignment and sound events as they are added.
ONNX Runtime itself is Microsoft's CUDA 13 build, loaded at run time from the runtime folder.

## Contents

```text
crates/inference/src/onnx/
├── mod.rs       the module list and the re-exports of `Device` and `OnnxError`
├── separation/  the separation models, their STFT and the streaming overlap-add
├── session.rs   opening a model on CUDA without a silent CPU fall-back, and describing its inputs
└── tests/       checks against the downloaded models, run on the host when asked for
```

## How it works

`session::open` builds an `ort` session with full graph optimisation and, for `Device::Cuda`, the
CUDA execution provider marked `error_on_failure`: when the CUDA libraries cannot be loaded the
open fails instead of running on the CPU. `ort` finds ONNX Runtime through `ORT_DYLIB_PATH`, and
ONNX Runtime finds its CUDA provider and the CUDA libraries through `LD_LIBRARY_PATH`; both come
from `crate::cuda_runtime::CudaRuntime::worker_env`, set by the process that starts the worker.

## Boundaries

- Depends on: `ort` 2.0.0-rc.13 with `load-dynamic` and `cuda`; `realfft` in `separation/`.
- Used by: `crates/stages/src/separation/` and `tools/stack_spike/`.
- Rules:
  - one `ort` version in the whole dependency tree (the coding standards; no gate holds it);
  - a CUDA session never falls back to the CPU (`error_on_failure` in `session.rs`; review);
  - the model checks in `tests/models.rs` are `#[ignore]` because they need the downloaded models
    and the host's GPU.

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#11-onnx-runtime-from-rust) — `ort`, its
  CUDA 13 needs and version clashes.
- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — the CUDA libraries ONNX Runtime needs on the host.
