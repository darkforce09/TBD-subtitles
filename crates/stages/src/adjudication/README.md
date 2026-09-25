# Adjudication stage

[Adjudication](/documentation/glossary.md#adjudication): the language model settles each
disagreement in the diff sheet under rules that stop it inventing words, and the answer is
checked automatically.

## Contents

```text
crates/stages/src/adjudication/
├── checks.rs  the checks: every id once, novel words, dropped agreed words, reading speed
├── mod.rs     `adjudicate` (one model) and `adjudicate_concurrently` (several processes), batches of 60
├── prompt.rs  the rules (system prompt), the answer's JSON Schema, the user message with the glossary
└── tests/     unit tests for the checks
```

## How it works

The sheet goes to the model in batches of 60 utterances, each with the glossary. Every answer is
`{"lines": [{"id", "t", "f"}]}`; ids a batch left out are asked for once more, and the lines are
put back in sheet order. `checks::check` then lists missing, duplicate and unknown ids; words no
engine heard in the utterance or next to it and the glossary lacks (`novel`); agreed non-filler
words the answer dropped, unless the line is flagged `DROP` or `LYRIC`; and lines reading faster
than 25 characters per second.

## Boundaries

- Depends on: `inference::llm::LanguageModel`, `crate::diff_sheet::{align, sheet}`, `serde`,
  `serde_json`.
- Used by: `tools/stack_spike/` (the language-model items).
- Rules:
  - the model never sees a timing (the header in `mod.rs`);
  - invented and dropped words are caught, and joined or hyphenated agreed words are not taken for
    dropped ones (`tests/checks.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#6-adjudication) — the output format, flags
  and checks.
