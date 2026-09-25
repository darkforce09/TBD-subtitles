# ONNX Runtime models

Models run through ONNX Runtime (the `ort` crate) on CUDA: speech recognition, vocal separation,
[CTC](/documentation/glossary.md#ctc) alignment and sound events. The module's code is not written
yet; `mod.rs` holds only its header.

## Contents

```text
crates/inference/src/onnx/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/inference/src/lib.rs` declares it as a public module.
- Rules: one `ort` version in the whole dependency tree (the coding standards; no gate holds it).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#11-onnx-runtime-from-rust) — `ort`, its
  CUDA 13 needs and version clashes.
- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — the CUDA libraries ONNX Runtime needs on the host.
