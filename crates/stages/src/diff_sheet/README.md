# Diff sheet stage

Every engine's words aligned to the backbone engine's, as the sheet the language model settles:
one line per utterance, with each disagreement written inline.

## Contents

```text
crates/stages/src/diff_sheet/
├── align.rs  word normalisation and the edit-distance alignment of two word lists, with error counts
├── mod.rs    the module list
├── sheet.rs  the sheet: utterances cut at pauses and sentence ends, variants inline, heard spans
└── tests/    unit tests for normalisation, alignment, the sheet's lines, its cuts and heard spans
```

## How it works

`sheet::build` aligns each other engine's words to the backbone's, chunk by chunk, on normalised
words. It cuts the backbone into utterances at pauses of 0.6 s, at sentence ends followed by
0.2 s, and at 12 s, and writes each as `U0412 12:03.4 2.1s | Law, the {P:Birdcage|W:bird cage}
is closing{W:+in}!`. A word every engine heard the same is locked; each utterance keeps every
engine's own words for the novelty check. The `Utterance` type is `job_model::outputs::Utterance`,
re-exported from `sheet`, so the pipeline stores the sheet as `outputs/diff_sheet` in the job's
database and the later stages read it back.

`sheet::heard_spans` cuts the same utterances through the same comparison and returns, per
utterance, the earliest start and latest end of its backbone words and of every other engine's
word it holds: words heard before the chunk's first backbone word (in the chunk's first
utterance), in a backbone word's place, or between its words. The sheet's own times stay the
backbone's; the heard spans let forced alignment look where another engine heard words the
backbone missed.

## Boundaries

- Depends on: `job_model::outputs` (`EngineTranscript`, `TimedWord`, `Utterance`).
- Used by: `crates/pipeline/src/tasks/speech.rs` (the diff-sheet step);
  `crates/pipeline/src/tasks/alignment.rs` (`heard_spans`, for the alignment and review steps);
  `crates/stages/src/adjudication/` (the sheet and the word normalisation) and
  `crates/stages/src/alignment/` (`align`, to line displayed words up with the backbone's);
  `crates/stages/src/fix_it/` (`align` for its change checks, `sheet::clock` for its times);
  `tools/stack_spike/`, `tools/stack_spike_ggml/` and `tools/stack_spike_llm/`.
- Rules: an alignment visits every word of both lists once, in order
  (`every_word_of_both_lists_appears_once_in_order`); disagreements are written inline and
  agreements locked (`disagreements_are_written_inline_and_agreements_locked`); there is one
  heard span per utterance of the sheet (`there_is_one_span_per_utterance_of_the_sheet`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#5-diff-sheet) — the sheet's format and rules.
