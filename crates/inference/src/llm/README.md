# Language-model backends

The language-model backends behind one trait: each takes the
[diff sheet](/documentation/glossary.md#diff-sheet) and returns the adjudicated utterances as JSON.
The trait and both backends are not written yet.

## Contents

```text
crates/inference/src/llm/
├── claude_cli/  the headless `claude -p` CLI: a JSON schema in, the structured answer out, no tools
├── mistral_rs/  a local model through mistral.rs on the GPU
└── mod.rs       the module header and the two backend declarations
```

## How it works

The [adjudication](/documentation/glossary.md#adjudication) stage is the one caller: it hands a
backend the diff sheet and takes back one JSON object per utterance. `claude_cli/` is for the
`claude` program, started as a child process; `mistral_rs/` is for a local model loaded inside the
worker process. `mod.rs` declares both; each holds only its header.

## Public surface

- `llm::claude_cli` and `llm::mistral_rs`: public modules with no items yet, for
  `crates/stages/src/adjudication/`.

## Boundaries

- Depends on: nothing yet; the module holds no code beyond its declarations.
- Used by: nothing; `crates/inference/src/lib.rs` declares it as a public module.
- Rules: every backend takes the diff sheet and answers with the adjudicated utterances as JSON,
  behind the one trait (the header in `mod.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#6-adjudication) — the adjudication input,
  answer and automatic checks.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#6-language-models) — the local models
  and the `claude` flags.
