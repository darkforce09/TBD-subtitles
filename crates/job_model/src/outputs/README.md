# Stage outputs

The typed output of each [stage](/documentation/glossary.md#stage), one JSON file per step in the
job's work directory, from the probe result to the aligned words.

## Contents

```text
crates/job_model/src/outputs/
├── adjudication.rs  the model's lines, the checks' findings, one pass, the unsure heard again
├── aligned.rs       the final words with their times and timing source, per kept utterance
├── mod.rs           the module list and the re-exports
├── probe.rs         the probe result, and the probe with its decoded audio track
├── sheet.rs         a diff-sheet utterance: backbone words, locks, the sheet line, every hypothesis
├── shots.rs         the shot changes: every scdet cut with its time and score
├── sound_cues.rs    the sound-cue candidates, the chosen and worded cues, the refused answers
├── sound_events.rs  a sound event: class, stem, times and peak probability
├── speech.rs        the speech plan: speech regions and the chunks the engines transcribe
└── words.rs         an engine's transcript: timed words per chunk, with the engine and its input
```

## How it works

Each type is the file one step writes and the steps after it read:

| File | Type | Written by |
|---|---|---|
| `probe.json` | `ProbeDecoded` (a `ProbeResult`, the `AudioStream` decoded, the samples) | probe and decode |
| `shots.json` | `ShotChanges` | the shot scan |
| `vad.json` | `SpeechPlan` | voice activity |
| `asr/<engine>.json` | `EngineTranscript` | speech recognition, one per engine |
| `sheet.json` | a list of `Utterance` | the diff sheet |
| `sound_events.json` | a list of `SoundEvent` | sound events |
| `adjudication/first.json` | `AdjudicationPass` | the first adjudication pass |
| `adjudication/redecode_<engine>.json` | `Redecode` | the re-decode, one per engine |
| `adjudicated.json` | `AdjudicationPass` | the second pass, merged with the first |
| `sound_cues.json` | `SoundCues` | the sound-cue choice |
| `aligned.json` | `Aligned` | forced alignment |

A `Line` is one adjudicated utterance as the model returns it: its id, its final text (`||` marks
a speaker change) and its flags (`NARR`, `LYRIC`, `DROP`, `UNSURE`). An `AlignedWord` records its
`TimingSource`, best first: `ctc`, `ctc_utterance`, `backbone` or `interpolated`. A
`SoundCandidate` is an `effect`, a `voice`, a Whisper `tag` or a `song`; a `SoundCue` is a chosen
candidate at the candidate's times with its bracketed text. The cue track in `cues.json` is
`subtitle_formats::cue::CueTrack`, and the quality check's `qc.json` is `crate::report::QcReport`.

## Boundaries

- Depends on: `serde` for the derives.
- Used by: `crates/media_io/` (the probe and the shot-change scan), `crates/inference/` (timed
  words), `crates/stages/` (every stage's inputs and outputs), `crates/pipeline/src/tasks/`, which
  reads and writes each file, and the stack spike tools.
- Rules: an output type changes only together with every stage that reads or writes it, and its
  JSON names stay stable so a resumed job reads what an earlier run wrote (the crate header in
  `crates/job_model/src/lib.rs`); shot changes keep every scdet score so the cue stage chooses the
  threshold (review).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — the file each step
  writes.
- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the job
  work directory.
