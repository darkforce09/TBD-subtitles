# Quality check stage

The quality check stage: the finished cues held against the layout, timing and coverage rules, the
lines the earlier steps flagged listed with a timestamp to look at, and the job report rendered for
the owner to read.

## Contents

```text
crates/stages/src/qc/
├── coverage.rs     stretches the backbone heard words in, and speech over 1 s that no cue covers
├── markdown.rs     `report.md`: the summary, the findings per check, then the step sections
├── mod.rs          `check`: cue findings, line findings, uncovered speech and the summary
├── steps_table.rs  the step table with speed, CPU, GPU and memory, the totals and the phase times
└── tests/          unit tests for each layout rule, coverage, the whole check and the report
```

## How it works

`check` takes a `QcInput` (the cue track, the aligned words, the adjudicated lines, the sound
cues, the speech plan, the stretches the backbone heard words in (`coverage::heard_spans`, each
word capped at 1 s), the utterances dropped as noise, and each utterance's start) and returns a
`QcReport`:

- `cue_findings`: every cue that is empty, holds more than two lines, has a line over 42
  characters, is shorter than the minimum or longer than 7 s, reads over 20 characters per second,
  overlaps or comes within 2 frames of the next, or ends after the video;
- `coverage::uncovered`: every stretch of heard speech of at least 1 s outside the cues, the
  songs and the dropped lines (Whisper's times and its laughs, which the language model drops,
  would flag speech that is covered);
- the line findings: every unsure utterance, every novel word and dropped agreed word, every
  utterance with less than half its words timed by the aligner, an aligner offset of 30 ms or
  more, and every failed language-model call;
- the summary: cues by kind, the share at or under 20 characters per second (target 95 %), words
  per timing source, the offset, the uncovered heard speech, and the voice activity with no cue
  (grunts and crowds included), for reference.

Findings come out in time order. `markdown::render` turns the report, the job record's step
records, the last run (`JobRun`) and the dropped sound cues into `report.md`. Its step sections
come from `steps_table`: one row per step with its wall, load and process time, its speed against
the video (`× RT`, the video's length over the wall time, computed rather than stored), the mean and
peak CPU cores, the busiest thread, GPU use, its own, its largest child's and the whole job's peak
memory and its VRAM; then the summed step time projected to a 120-minute video, the last run's real
wall time and the whole job's peak memory; then `## Phase times` for the steps that note them (the
text detection's and the localized video's phases and frame rates, the localized video's
segments re-encoded and frames copied or why the whole video was re-encoded, the shot scan's and
the read-back check's speed). A measure that was not taken shows as `—`, never as 0.

## Boundaries

- Depends on: `crate::cues` (`FrameRules`, `line_break::MAX_LINE`, `segment::MAX_CPS`),
  `crate::sound_events::candidates::sound_tag` (Whisper's sound tags kept out of heard speech),
  `subtitle_formats::cue`, `job_model::outputs`, `job_model::report` (`QcCheck`, `QcFinding`,
  `QcSummary`, `QcReport`), `job_model::job::{JobRecord, JobRun, StepMeasure, StepRecords}` and
  `job_model::StepName`.
- Used by: `crates/pipeline/src/tasks/layout.rs` (the QC step, which stores `outputs/qc`) and
  `crates/pipeline/src/report/mod.rs` (which renders `report.md`).
- Rules:
  - the stage runs inside the job runner, not in a worker, so it loads no model and starts no
    child process (`placement` in `crates/pipeline/src/graph/mod.rs`);
  - a clean track has no cue findings and each layout rule is flagged
    (`a_clean_track_has_no_cue_findings`, `each_layout_rule_is_flagged` in `tests/qc.rs`);
  - speech without a cue is found, except in songs and dropped noise
    (`speech_without_a_cue_is_found_outside_songs_and_dropped_noise`);
  - an unmeasured value shows as a dash in the report
    (`the_report_lists_flags_steps_and_unmeasured_values_as_dashes` in `tests/markdown.rs`);
  - the footer gives the last run's real wall time and the whole job's peak memory, and the phase
    section lists only the phases a step noted
    (`the_footer_gives_the_real_wall_time_and_the_whole_job_peak`,
    `the_phase_section_splits_detection_and_the_localized_video` in `tests/steps_table.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — the checks and the report.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — the rules the cue
  findings hold.
