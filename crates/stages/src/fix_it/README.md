# Fix It stage

[Fix It](/documentation/glossary.md#fix-it): on a finished job, a stronger `claude` model fixes the
lines the quality check flagged. It reads the whole video once to work out its context by itself,
fixes one problem family at a time, and a judge keeps a change only when it reads right against
the line before it. A guard holds every proposal to what the speech engines heard.

## Contents

```text
crates/stages/src/fix_it/
├── brief.rs     the first pass: the context from the names, glossary and every line; suspects
├── calls.rs     several calls at once, none once stopped; calls, tokens, cost and failures
├── change.rs    `changed_words`: the first run of words a recorded change touched, as written
├── evidence.rs  what the model reads about a line: place, engines, line now, timing, neighbours
├── guard.rs     the rules each proposal must keep; the heard words a change leaves out
├── items.rs     which findings Fix It asks about, in which family, in words; what was answered
├── judge.rs     the last pass: each change accepted or turned down against the line before it
├── mod.rs       `run`, `Episode`, `Pass`, `FixRun`, `FixFailure` and the drafts of asked lines
├── prompt.rs    the system prompts of the brief, each family and the judge, and their schemas
├── repair.rs    the second pass: one family's lines in batches of 25, each answer guarded
└── tests/       unit tests with a scripted model and a video modelled on Dressrosa 12
```

## How it works

```text
names, glossary, every line ─▶ brief ─▶ items (findings + suspects)
                                               │
      words ─▶ timing and layout ─▶ reading speed   (repair, each answer through the guard)
                                               │
                        judge (changed lines only) ─▶ FixRun: one LineFix per asked line
```

**Brief.** The owner types nothing. One call reads the video's file name and folder (such as
`[Muhn Pace] Dressrosa 12` in `one_pace`), the glossary's name and terms, and every line as
`ID m:ss.d [FLAGS] | text`, and uses what the model knows of the series to name the show, the
episode and the cast, sum up the scenes, list speech habits that are meant (a stutter, a
catchphrase), and name up to 30 lines that do not fit the conversation. A video of more than 800
lines is read in parts, each with the summaries before it. The brief opens every later message.

**Items.** `asks_about` sorts each finding into a family: unsure lines, words no engine heard and
heard words replaced are *words*; speech with no subtitle, loosely timed lines and every layout
rule are *timing and layout*; subtitles over 20 characters per second are *reading speed*. The
aligner's offset and failed calls are not Fix It's. A line the owner settled is never asked about,
and a Fix It change the owner has not checked is not asked about its words again. `Answered`,
made from the earlier runs' `FixRecord`, holds each line whose verdict answered it
(`FixVerdict::answered`: unchanged, kept, accepted or turned down) with the checks it was asked
about; a finding it covers (the same line and check) is not asked again, while a new check on
the same line is, and a line recorded with no checks covers every check. Speech with no subtitle
is never covered: it asks about the unsettled lines within 0.5 s of it, at most three, with the
words the main engine heard in it. The brief's suspects join the word problems, unless the line
has a correction or an earlier run answered it. Each item carries the checks behind its
problems, and the line's `LineFix::checks` gathers them.

**Repair.** Each family asks about its lines in batches of 25, several calls at once, words
first, so each later family sees the lines as the earlier ones left them. Each block shows the
line's start and length and the gaps around it, what each engine heard (`P`, `W`, and `p`, `w`
heard again), the line now, its timing with a `~` before each word the aligner could not place,
and three lines on each side; never a word's time. Returning a line as it is keeps it: for timing
and layout it is then timed again alone, and for an unsure line or a heard word replaced the
model confirms it. A line in a batch whose call failed, or whose answer does not match the
schema, that ends with its words as they were and nothing kept is `NotAnswered`, with the
failure `Calls` recorded last, so the next run asks about it again.

**Guard.** A proposal needs a reason, known flags (`NARR`, `SPK`, `LYRIC`, `DROP`), text unless
it drops the line, no sheet notation, and no word `adjudication::checks::check` calls novel: every
word was heard by an engine in the line or the ones next to it, or the glossary has it. A
reading-speed fix may only take words out and keeps the flags. A refused proposal is recorded
and the line stays as it was.

**Judge.** Every changed line goes to the judge, 40 to a call, with its problems, before and
after, each family's reason, the heard words it now leaves out, what the engines heard and the
lines around it. A change the judge accepts is kept; one it turns down, leaves out, or whose call
failed is not.

## Public surface

- `run`, `Episode`, `Pass`, `FixRun`, `FixFailure`, `Make` and `Usage`: one Fix It run, for
  `crates/pipeline/src/fix_it/`.
- `items::asks_about` and `items::Answered`: whether Fix It asks about a finding, given what
  earlier runs answered, for the window's report, so the button and the run agree.
- `changed_words` (from `change`): the first run of words a recorded change put in, took out or
  replaced, before then after, for the window.
- `brief`, `evidence`, `guard`, `judge`, `prompt` and `repair`: the passes and their rules.

## Boundaries

- Depends on: `inference::llm::{LanguageModel, purpose}` (each call is "{label}, call i of n"),
  `tracing` (the call threads keep the caller's span), `crate::adjudication::checks`,
  `crate::diff_sheet::{align, sheet}`, `job_model::outputs` (`FixRecord`'s parts, `Corrections`,
  `Line`, `Utterance`, `Aligned`, `EngineTranscript`), `job_model::report`, `serde`,
  `serde_json`.
- Used by: `crates/pipeline/src/fix_it/`; the app's report in
  `apps/tbd_subtitles/src/job_report/services/` (`items::asks_about`, `items::Answered` and
  `changed_words`).
- Rules:
  - no word no engine heard reaches a change (`a_word_no_engine_heard_is_refused_and_the_line_stays`
    in `tests/fix_it.rs`);
  - a line the owner settled is never asked about
    (`a_line_the_owner_settled_is_never_asked_about`);
  - a finding an earlier run answered is not asked again
    (`answered_findings_are_not_asked_again` in `tests/items.rs`);
  - a line whose call failed is not answered
    (`a_failed_repair_call_leaves_its_lines_as_they_were_and_not_answered`);
  - only a change the judge accepts is kept (`only_the_judge_s_accepted_changes_are_kept`);
  - a stopped run makes no further call (`a_stopped_run_makes_no_further_call`);
  - the context comes only from the job and the model, never from the owner
    (`the_brief_reads_the_names_the_glossary_and_every_line`).

## Related documentation

- [Fix It](/documentation/features/fix_it.md) — the feature: the button, what it changes, and
  how the owner keeps or undoes each change.
- [Pipeline](/documentation/architecture/pipeline.md#fix-it) — where Fix It sits after a finished
  job.
