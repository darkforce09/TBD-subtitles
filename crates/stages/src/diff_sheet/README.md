# Diff sheet stage

Every engine's words aligned to the backbone engine's, as the sheet the language model settles.
The word alignment it is built on is written; the sheet itself follows.

## Contents

```text
crates/stages/src/diff_sheet/
├── align.rs  word normalisation and the edit-distance alignment of two word lists, with error counts
├── mod.rs    the module list
└── tests/    unit tests for normalisation, alignment coverage and error counts
```

## Boundaries

- Depends on: nothing outside `std`.
- Used by: `tools/stack_spike/` (the engine comparison).
- Rules: an alignment visits every word of both lists once, in order
  (`every_word_of_both_lists_appears_once_in_order`); case, punctuation and number words fold
  before comparing (`case_punctuation_and_number_words_fold`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#5-diff-sheet) — the sheet's format and rules.
