# Forced alignment stage

[Forced alignment](/documentation/glossary.md#forced-alignment) of the final text against the
vocal stem: the kept utterances grouped into blocks, every displayed word timed from a CTC grid,
each result checked, and a fallback for every block or utterance that fails, so every word leaves
with a time and the source of that time.

## Contents

```text
crates/stages/src/alignment/
├── blocks.rs       kept utterances, `||` speaker indices, blocks of 20–60 s at pauses, audio spans
├── checks.rs       an alignment against reference times: median start difference, share over 200 ms, runs
├── ctc_viterbi.rs  the best path of a known token sequence through a CTC grid, with each token's frames
├── mod.rs          `align_words_ctc`: displayed words ─▶ spoken tokens ─▶ Viterbi ─▶ word times
├── run.rs          the `WordAligner` trait and `align_all`: blocks, fallbacks, the offset
├── spoken_form.rs  displayed words to spoken words: accents, digits, hyphens, symbols, punctuation
├── timing.rs       backbone times, interpolation, the pass checks and the signed median
└── tests/          unit tests for blocks, Viterbi, the spoken form, the checks, timing and the run
```

## How it works

`blocks::kept` turns the final lines into the utterances to align: every line not flagged `LYRIC`
or `DROP` and not empty, its `||` marks taken out of the text and kept as the indices where
another speaker starts, with its `NARR` and `UNSURE` flags and the backbone engine's timed words.
`plan_blocks` groups them into blocks: a block closes at a pause of 0.35 s once it is 20 s long or
would pass 60 s, at any gap before it would pass 90 s, and always where a lyric or dropped
utterance lies between two kept ones. `audio_span` pads a block's speech by 0.3 s, never past the
middle of the gap to its neighbour.

`run::align_all` hands each block to a `WordAligner`, which the pipeline implements by running
Parakeet-CTC over the span and `align_words_ctc` over its grid. `align_words_ctc` turns each
displayed word into spoken words, spells each in the model's tokens (a closure the caller passes),
and runs `ctc_viterbi::align` over the whole sequence; a word runs from its first token's first
frame to its last token's last frame, and a word with nothing speakable gets no time.

`timing::passes` judges each result: no run of three zero-length words (`checks::flat_runs`),
every utterance's speech within 1 s of its recognition window, and a median start difference from
the backbone's times (`backbone_times`, through the diff sheet's word alignment) of at most
0.2 s. Each word records where its time came from:

```text
block aligned and passes ─────────────────────────────▶ ctc
block fails ─▶ utterance aligned alone and passes ────▶ ctc_utterance
          └──▶ utterance fails ─▶ backbone word time ─▶ backbone
any word still untimed ─▶ spread by characters between its neighbours ─▶ interpolated
```

`timing::signed_median` of aligner start minus backbone start, over words timed in passing
blocks, is the job's offset. `checks::suspicious_runs` also counts words spread at even steps, for
the aligner comparisons of the stack spike tools.

## Boundaries

- Depends on: `crate::diff_sheet::align` (word matching against the backbone),
  `job_model::outputs` (`Line`, `Utterance`, `TimedWord`, `TimeSpan`, and the `Aligned` types it
  returns); the CTC grid and tokenizer come from the caller.
- Used by: `crates/pipeline/src/tasks/alignment.rs` (the alignment step, with
  `inference::onnx::parakeet_ctc`), `tools/stack_spike/` (CTC alignment) and
  `tools/stack_spike_ggml/` (the checks, for the Qwen3 aligner).
- Rules:
  - tokens keep their order, never overlap and each gets a frame; too short a grid is no
    alignment (`tests/ctc_viterbi.rs`);
  - evenly spread and zero-length runs are flagged, real speech is not
    (`evenly_spread_words_are_flagged_and_real_speech_is_not`);
  - lyric, dropped and empty lines are not kept, and a block closes at them
    (`lyric_dropped_and_empty_lines_are_not_kept`,
    `blocks_close_at_pauses_after_twenty_seconds_and_at_dropped_lines` in `tests/blocks.rs`);
  - an alignment outside its window, far from the backbone or collapsed fails
    (`an_alignment_fails_outside_its_window_far_from_the_backbone_or_collapsed` in
    `tests/timing.rs`);
  - a failed block falls back to utterances, then to the backbone, and every word gets a time in
    order (`a_failed_block_falls_back_to_utterances_then_to_the_backbone`,
    `every_word_gets_a_time_in_order` in `tests/run.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#7-forced-alignment) — blocks, pass rules and
  fallbacks.
