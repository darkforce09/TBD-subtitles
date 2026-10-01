# Stage outputs

The typed output of each [stage](/documentation/glossary.md#stage), one document per step in the
job's database, from the probe result to the output's record, with the owner's corrections and Fix
It's record.

## Contents

```text
crates/job_model/src/outputs/
├── adjudication.rs  the model's lines, the checks' findings, one pass, the unsure heard again
├── aligned.rs       the final words with their times and timing source, per kept utterance
├── fix_it.rs        Fix It's record: the brief, each line asked about, its changes and verdict
├── mod.rs           the module list and the re-exports
├── output.rs        what the output step left: the subtitle file, its backup, a retired format
├── probe.rs         the probe result, and the probe with its decoded audio track
├── review.rs        the corrections: each utterance's text, flags and where it came from
├── sheet.rs         a diff-sheet utterance: backbone words, locks, the sheet line, every hypothesis
├── shots.rs         the shot changes: every scdet cut with its time and score
├── sound_cues.rs    the sound-cue candidates, the chosen and worded cues, the refused answers
├── sound_events.rs  a sound event: class, stem, times and peak probability
├── speech.rs        the speech plan: speech regions and the chunks the engines transcribe
├── tests/           unit tests for the Fix It record, the corrections, every output's rkyv form
└── words.rs         an engine's transcript: timed words per chunk, with the engine and its input
```

## How it works

Each type is the document one step writes and the steps after it read, in the job's database
(`outputs/<step>`):

| Document | Type | Written by |
|---|---|---|
| `outputs/probe_decode` | `ProbeDecoded` (a `ProbeResult`, the `AudioStream` decoded, the samples) | probe and decode |
| `outputs/shot_scan` | `ShotChanges` | the shot scan |
| `outputs/vad` | `SpeechPlan` | voice activity |
| `outputs/asr_parakeet`, `outputs/asr_whisper` | `EngineTranscript` | speech recognition, one per engine |
| `outputs/diff_sheet` | a list of `Utterance` | the diff sheet |
| `outputs/sound_events` | a list of `SoundEvent` | sound events |
| `outputs/adjudicate` | `AdjudicationPass` | the first adjudication pass |
| `outputs/redecode_parakeet`, `outputs/redecode_whisper` | `Redecode` | the re-decode, one per engine |
| `outputs/readjudicate` | `AdjudicationPass` | the second pass, merged with the first |
| `outputs/sound_cues` | `SoundCues` | the sound-cue choice |
| `outputs/alignment` | `Aligned` | forced alignment |
| `corrections/lines` | `Corrections` | the window's line review, and Fix It |
| `corrections/fix` | `FixRecord` | Fix It |
| `outputs/review` | `Aligned` | the review step |
| `outputs/output` | `OutputRecord` | the output step |

A `Line` is one adjudicated utterance as the model returns it: its id, its final text (`||` marks
a speaker change) and its flags (`NARR`, `LYRIC`, `DROP`, `UNSURE`). An `AlignedWord` records its
`TimingSource`, best first: `ctc`, `ctc_utterance`, `backbone` or `interpolated`. A
`SoundCandidate` is an `effect`, a `voice`, a Whisper `tag` or a `song`; a `SoundCue` is a chosen
candidate at the candidate's times with its bracketed text. The cue track in `outputs/cues` is
`subtitle_formats::cue::CueTrack`, and the quality check's `outputs/qc` is `crate::report::QcReport`.

A `Correction` says where its text came from in `Chosen`: an engine's hypothesis, text the owner
typed, a Fix It change the owner has not checked yet (`fix_it`, with the model and its reason), or
one the owner kept (`kept_fix_it`). Every choice but an unchecked Fix It change is the owner's
(`Correction::by_owner`). A `FixRecord` keeps the Fix It runs of a video since it was last
adjudicated: the brief the model worked out from the video's names and lines, and for each line
asked about its problems and their `QcCheck`s, each change that passed the guard (`FixStep`), the
proposals refused, the heard words left out, the `FixVerdict` and whether the correction was
applied. `FixVerdict::answered` says whether a verdict settles the line's checks (not for
`not_judged` or `not_answered`, a line whose every call failed). The record also keeps the
problems before the first run (`FixBefore::of` the quality check) and the re-adjudication
fingerprint the runs read; `FixRecord::is_current` is false once the job's differs.

## Boundaries

- Depends on: `serde` and `rkyv` for the derives; `crate::report` (`QcCheck`, `QcReport`) for Fix
  It's record.
- Used by: `crates/media_io/` (the probe and the shot-change scan), `crates/inference/` (timed
  words), `crates/stages/` (every stage's inputs and outputs), `crates/pipeline/` (`src/tasks/`
  reads and stores each document, `src/fix_it/` the corrections and Fix It's record), the app's
  window and the stack spike tools.
- Rules: an output type changes only together with every stage that reads or writes it, and a
  changed type bumps the layout version of the table that stores it (the crate header in
  `crates/job_model/src/lib.rs`); corrections written before Fix It still read
  (`an_earlier_review_json_still_reads` in `tests/review.rs`); shot changes keep every scdet
  score so the cue stage chooses the threshold (review).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — what each step
  stores.
- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the job
  work directory.
