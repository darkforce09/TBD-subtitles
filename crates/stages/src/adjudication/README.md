# Adjudication stage

[Adjudication](/documentation/glossary.md#adjudication): the language model settles each
disagreement in the diff sheet under rules that stop it inventing words, hears the
[unsure](/documentation/glossary.md#unsure) utterances again with fresh hypotheses, and chooses and
words the [sound cues](/documentation/glossary.md#sound-cue); every answer is checked
automatically.

## Contents

```text
crates/stages/src/adjudication/
├── checks.rs      the checks: every id once, novel words, dropped agreed words, reading speed
├── glossary/      the built-in One Piece glossary and the reader for glossary files
├── mod.rs         the first pass: `adjudicate` and `adjudicate_concurrently`, batches of 60
├── prompt.rs      the rules, the answer's JSON Schema, the user message with the glossary
├── redecode.rs    unsure spans heard again, their alternatives, the second pass and the merge
├── sound_cues.rs  the model chooses sound candidates and words them; the checks on each cue
├── summary.rs     a run summed up: check counts, flags, changed lines, glossary name counts
└── tests/         unit tests for the checks, the re-decode, the sound cues and the name counting
```

## How it works

The first pass sends the sheet to the model in batches of 60 utterances, each with the glossary.
Every answer is `{"lines": [{"id", "t", "f"}]}`, with the flags `NARR`, `SPK` (the utterance starts
with a different speaker than the one before), `LYRIC`, `DROP` and `UNSURE`; ids a batch left out
are asked for once more, and the lines are put back in sheet order. Both `adjudicate` and
`adjudicate_concurrently` report each finished batch to a progress callback. One private `ask_with` makes every call with a given system
prompt and message, so the first and second passes share the answer reading and the cost counting.
`checks::check` then lists missing, duplicate and unknown ids; words no engine heard in the
utterance or next to it and the glossary lacks (`novel`); agreed non-filler words the answer
dropped, unless the line is flagged `DROP` or `LYRIC`; and lines reading faster than 25 characters
per second.

`redecode.rs` takes the ids the first pass flagged `UNSURE` and gives the pipeline their spans,
padded by 0.5 s on each side and clamped to the video, for both engines to hear again on the vocal
stem. `with_alternatives` adds what they heard to those utterances as extra hypotheses and shows it
on the line as `ALT p: "…" w: "…"`. `readjudicate` asks the model about those ids only, with each
one's settled neighbours as `CONTEXT` lines, asks once more for ids the answer left out, and drops
any line for an id it did not ask, reporting each batch asked; `merge` puts the second pass's lines in place of the first's.

`sound_cues.rs` groups the sound candidates into windows of 300 s of video, shows each window's
candidates among the dialogue within 10 s of them, and asks several `claude` processes at once.
A choice is kept only when it names a known candidate, is its first use, and is one bracketed
phrase, lowercase except words the glossary holds. A kept cue takes its candidate's times; every
song candidate becomes a music cue, worded by the model or `[music playing]`.

```text
sheet ─▶ adjudicate ─▶ first pass ─▶ unsure_ids ─▶ spans ─▶ (engines hear again)
                                                                  │
adjudicated      ◀─ merge ◀─ readjudicate ◀─ with_alternatives ◀──┘
        │
        └─▶ (candidates from sound_events) ─▶ sound_cues::choose ─▶ sound cues
```

## Public surface

- `adjudicate`, `adjudicate_concurrently`, `Adjudication` and `BATCH`: the first pass, for the
  adjudicate step in `crates/pipeline/src/tasks/llm.rs` and the stack spike tools.
- `checks::{check, Line, Findings}`: the checks; `Line` and `Findings` are
  `job_model::outputs` types, re-exported here.
- `redecode::{unsure_ids, spans, with_alternatives, readjudicate, merge}`: the re-decode plan for
  `crates/pipeline/src/tasks/speech.rs` and the second pass for `crates/pipeline/src/tasks/llm.rs`.
- `sound_cues::{choose, settle, check_choice}`: the sound-cue choice, for
  `crates/pipeline/src/tasks/sounds.rs`.
- `glossary::{one_piece, parse, as_strs}`: the glossary, for the app's `process` subcommand and the
  stack spike tool.
- `prompt` and `summary`: the rules and the run summary, for the stack spike tools.

## Boundaries

- Depends on: `inference::llm::{LanguageModel, purpose}` (each call says why it is made: the
  batch, the round of words heard again, or the sound window), `crate::diff_sheet::{align, sheet}`,
  `job_model::outputs` (`Line`, `Findings`, `Redecode`, `SoundCandidate`, `SoundCue`,
  `SoundCues`), `serde`, `serde_json`.
- Used by: `crates/pipeline/src/tasks/` (`llm.rs`, `speech.rs`, `sounds.rs`),
  `crates/stages/src/fix_it/` (the checks, for its guard), `crates/pipeline/src/fix_it/` (the
  re-decoded alternatives),
  `apps/tbd_subtitles/src/cli/process_command.rs` (the glossary), `tools/stack_spike/` and
  `tools/stack_spike_llm/`.
- Rules:
  - the model never sees a timing (the header in `mod.rs`);
  - invented and dropped words are caught, and joined or hyphenated agreed words are not taken for
    dropped ones (`tests/checks.rs`);
  - only unsure lines are heard again and asked again, and only their lines are replaced
    (`only_unsure_lines_are_heard_again_with_padding_inside_the_video`,
    `merging_replaces_only_the_answered_ids` in `tests/redecode.rs`);
  - a sound cue is one bracketed lowercase phrase for a known candidate, takes its candidate's
    times, and every song gets a music cue
    (`a_cue_must_be_one_bracketed_lowercase_phrase_for_a_known_candidate`,
    `settled_cues_take_candidate_times_and_every_song_gets_a_music_cue` in `tests/sound_cues.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#6-adjudication) — the output format, flags,
  checks, the re-decode and the sound-cue choice.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#sdh-sound-cues) — how
  a sound cue is worded.
