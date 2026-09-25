# Diff sheet stage

Every engine's words aligned to the backbone engine's, as the sheet the language model settles:
one line per utterance, with each disagreement written inline.

## Contents

```text
crates/stages/src/diff_sheet/
├── align.rs  word normalisation and the edit-distance alignment of two word lists, with error counts
├── mod.rs    the module list
├── sheet.rs  the sheet: utterances cut at pauses and sentence ends, variants inline, locked words
└── tests/    unit tests for normalisation, alignment, the sheet's lines and its cuts
```

## How it works

`sheet::build` aligns each other engine's words to the backbone's, chunk by chunk, on normalised
words. It cuts the backbone into utterances at pauses of 0.6 s, at sentence ends followed by
0.2 s, and at 12 s, and writes each as `U0412 12:03.4 2.1s | Law, the {P:Birdcage|W:bird cage}
is closing{W:+in}!`. A word every engine heard the same is locked; each utterance keeps every
engine's own words for the novelty check.

## Boundaries

- Depends on: `job_model::outputs`, `serde`.
- Used by: `crates/stages/src/adjudication/`, `tools/stack_spike/` and `tools/stack_spike_ggml/`.
- Rules: an alignment visits every word of both lists once, in order
  (`every_word_of_both_lists_appears_once_in_order`); disagreements are written inline and
  agreements locked (`disagreements_are_written_inline_and_agreements_locked`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#5-diff-sheet) — the sheet's format and rules.
