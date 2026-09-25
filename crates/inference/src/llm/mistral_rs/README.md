# Mistral.rs backend

A local language model through mistral.rs on the GPU as a backend. The module's code is not written
yet; `mod.rs` holds only its header.

## Contents

```text
crates/inference/src/llm/mistral_rs/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/inference/src/llm/mod.rs` declares it as a public module.
- Rules: none of its own beyond the language-model backends'; see the
  [backends README](/crates/inference/src/llm/README.md#boundaries).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#6-language-models) — mistral.rs and the
  models that fit the GPU.
