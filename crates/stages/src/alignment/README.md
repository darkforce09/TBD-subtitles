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
├── run.rs          `WordAligner`, `align_all` (blocks, fallbacks, offset), `realign_utterance`
├── spoken_form.rs  displayed words to spoken words: accents, digits, hyphens, symbols, punctuation
├── timing.rs       backbone times, carried times, interpolation, the pass checks, the signed median
└── tests/          unit tests for blocks, Viterbi, the spoken form, the checks, timing and the run
```

## How it works

`blocks::kept` turns the final lines into the utterances to align: every line not flagged `LYRIC`
or `DROP` and not empty, its `||` marks taken out of the text and kept as the indices where
another speaker starts, with its `NARR` and `UNSURE` flags and the backbone engine's timed words.
Its recognition window is the backbone's first to last word, widened where another engine heard
the line's words: displayed words before the first that matches a backbone word move the start
back to the heard span's start (`diff_sheet::sheet::heard_spans`), displayed words after the last
match move the end on to the heard span's end, and a widened edge stops at the neighbouring kept
window. A line only Whisper heard the start of, such as a heckle before an answer the backbone
heard, is so aligned where Whisper heard it. `plan_blocks` groups them into blocks: a block closes at a pause of 0.35 s once it is 20 s long or
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
the aligner comparisons of the stack spike tools. `run::realign_utterance` times one corrected
utterance alone over its own span, with the same checks; when the aligner fails it, the times
the aligner gave the line before carry over (`timing::carried_times`: a word that stayed keeps
its time, words that replaced others share their span, a new word is spread between its
neighbours), and only a line the aligner never timed falls back to the backbone's times; it
marks the line settled, and the pipeline's review step calls it.

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
  - a window widens only where displayed words lead or trail the backbone, to where another
    engine heard them, and never into a neighbouring kept window
    (`a_line_only_another_engine_heard_the_start_of_is_aligned_where_it_heard_it`,
    `a_line_the_backbone_covers_keeps_the_backbone_window`,
    `a_widened_window_stops_at_the_next_kept_window`);
  - an alignment outside its window, far from the backbone or collapsed fails
    (`an_alignment_fails_outside_its_window_far_from_the_backbone_or_collapsed` in
    `tests/timing.rs`);
  - a failed block falls back to utterances, then to the backbone, and every word gets a time in
    order (`a_failed_block_falls_back_to_utterances_then_to_the_backbone`,
    `every_word_gets_a_time_in_order` in `tests/run.rs`);
  - a corrected line is timed alone between its neighbours and settled, or keeps the backbone's
    times (`a_corrected_line_is_timed_alone_between_its_neighbours`,
    `a_corrected_line_the_aligner_fails_keeps_the_backbone_times`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#7-forced-alignment) — blocks, pass rules and
  fallbacks.
