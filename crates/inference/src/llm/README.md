# Language-model backends

The language-model backends behind one trait: a system prompt, a user message and a JSON Schema
in, a JSON value out. The adjudication stage uses them to settle the
[diff sheet](/documentation/glossary.md#diff-sheet). The `claude -p` backend is written; the local
mistral.rs backend is not.

## Contents

```text
crates/inference/src/llm/
├── claude_cli/  the headless `claude -p` CLI: a JSON schema in, the structured answer out, no tools
├── mistral_rs/  a local model through mistral.rs on the GPU; not written
└── mod.rs       the `LanguageModel` trait, `Completion` with tokens and cost, and `LlmError`
```

## How it works

`LanguageModel::complete_json` returns a `Completion`: the JSON answer, the input and output
tokens, and the provider's cost figure when it gives one. A backend that cannot produce JSON
matching the schema returns `LlmError`, never an empty answer.

## Boundaries

- Depends on: `serde_json`; `child_process` in `claude_cli/`.
- Used by: `crates/stages/src/adjudication/` and `tools/stack_spike/`.
- Rules: a backend never sees timings (the adjudication prompt carries none; the header in
  `crates/stages/src/adjudication/mod.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#6-language-models) — the backends and
  the Claude CLI flags.
