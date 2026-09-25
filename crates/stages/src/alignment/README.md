# Forced alignment stage

[Forced alignment](/documentation/glossary.md#forced-alignment) of the final text against the
vocal stem: every displayed word timed from a CTC grid, and the checks that catch an aligner
that failed quietly.

## Contents

```text
crates/stages/src/alignment/
├── checks.rs       an alignment against reference times: median start difference, share over 200 ms, flat runs
├── ctc_viterbi.rs  the best path of a known token sequence through a CTC grid, with each token's frames
├── mod.rs          `align_words_ctc`: displayed words ─▶ spoken tokens ─▶ Viterbi ─▶ word times
├── spoken_form.rs  displayed words to spoken words: accents, digits, hyphens, symbols, punctuation
└── tests/          unit tests for the Viterbi path, the spoken form and the checks
```

## How it works

`align_words_ctc` turns each displayed word into spoken words, spells each in the model's tokens
(a closure the caller passes, so this module needs no model), and runs `ctc_viterbi::align` over
the whole token sequence. A word's time runs from its first token's first frame to its last
token's last frame; a word with nothing speakable gets none. `checks::suspicious_runs` counts runs
of three or more words that are zero-length or spread at even steps, the signature of an aligner
that gave up.

## Boundaries

- Depends on: nothing outside `std`; the CTC grid and tokenizer come from the caller
  (`inference::onnx::parakeet_ctc` in `tools/stack_spike/`).
- Used by: `tools/stack_spike/` (CTC alignment) and `tools/stack_spike_ggml/` (the checks, for
  the Qwen3 aligner).
- Rules:
  - tokens keep their order, never overlap and each gets a frame; too short a grid is no
    alignment (`tests/ctc_viterbi.rs`);
  - evenly spread and zero-length runs are flagged, real speech is not
    (`evenly_spread_words_are_flagged_and_real_speech_is_not`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#7-forced-alignment) — blocks, pass rules and
  fallbacks.
