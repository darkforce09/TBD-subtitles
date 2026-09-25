# Claude CLI backend

The headless `claude -p` CLI as a language-model backend: a JSON schema in, the structured answer
out, with no tools enabled. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/inference/src/llm/claude_cli/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/inference/src/llm/mod.rs` declares it as a public module.
- Rules: the CLI runs with no tools enabled and answers against a JSON schema (the header in
  `mod.rs`); like every program the crates start, it runs through `crates/child_process/`.

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#6-language-models) — the `claude -p`
  flags and where the answer lands.
- [Pipeline](/documentation/architecture/pipeline.md#6-adjudication) — what the backend is asked.
