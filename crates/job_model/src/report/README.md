# Job report

The quality check's result, as kept in `qc.json` and rendered into `report.md`: every check by
name, the findings each leaves with a timestamp to look at, and the counts that sum a job up.

## Contents

```text
crates/job_model/src/report/
├── mod.rs  `QcCheck`, `QcFinding`, `QcSummary`, `QcReport` and `CPS_TARGET`
└── tests/  unit tests for the pass rule, a finding without an utterance and the rkyv round trips
```

## How it works

`QcCheck` names each problem the check looks for: the layout and timing rules of a cue (overlap,
gap, too short, too long, too fast, a long line, too many lines, empty, past the end), uncovered
speech, unsure utterances, novel words, dropped agreed words, weak timing, the aligner's offset
and failed language-model calls. `describe` gives each its phrase for the report, and
`is_layout_violation` marks the ones the success criteria forbid outright; the others are for
review. A `QcFinding` holds the check, the time in video seconds, the cue or utterance text and
the numbers behind it, and the utterance it is about when it is about one line of speech (the
window opens that line for review). `QcSummary` counts cues by kind, the share at or under 20
characters per second, words per timing source, the unsure and novel lines, the offset and the uncovered speech.
`QcReport::counts` gives findings per check. `QcReport::failures` lists why a job does not pass:
a layout violation, heard speech with no cue, a failed language-model call, an aligner offset of
30 ms or more, or fewer than `CPS_TARGET` (95 %) of cues at or under 20 characters per second;
`passes` is true when the list is empty. The other findings are for review.

## Boundaries

- Depends on: `serde`.
- Used by: `crates/stages/src/qc/`, which builds the report and renders it; `crates/pipeline/`
  (`src/runner/mod.rs` and `src/report/mod.rs`), which store and read it; the app's job report
  view and job queue, which show it and whether it passes.
- Rules:
  - the check names stay stable in JSON, so a resumed job reads what an earlier run wrote (the
    header in `mod.rs` and the crate header in `crates/job_model/src/lib.rs`), and a finding
    written without an utterance parses (`a_finding_without_an_utterance_parses` in
    `tests/report.rs`);
  - findings for review never fail a job (`findings_for_review_never_fail_a_job`), and each
    failing rule is named once
    (`layout_violations_uncovered_speech_failed_calls_and_offset_fail_a_job`,
    `too_few_cues_within_the_reading_speed_fail_a_job`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — the checks the report
  records.
- [Desktop GUI](/documentation/features/gui.md) — where the report is shown.
