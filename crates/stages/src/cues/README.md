# Cue building stage

The cue building stage: the aligned words and the chosen sound cues laid out as subtitle
[cues](/documentation/glossary.md#cue) by the Netflix English rules, timed on whole frames of the
source video and snapped to [shot changes](/documentation/glossary.md#shot-change).

## Contents

```text
crates/stages/src/cues/
├── line_break.rs  one line up to 42 characters, else the best-reading two-line break
├── mod.rs         `FrameRules` (the rules in frames), `Draft`, and `build`, which runs the passes
├── segment.rs     utterances cut into units that fit one cue; quick exchanges paired with dashes
├── shots.rs       the shot cuts that count, merged within 0.5 s, and cue edges snapped onto them
├── sound.rs       sound and music cues placed in free stretches, as a second line, or dropped
├── tests/         unit tests for each pass and for a whole scene built end to end
└── timing.rs      lead-in and lead-out, duration and reading speed, gaps, chaining, clamping
```

## How it works

`FrameRules::new` turns the rules into frames for the video's frame rate: a cue starts 1 frame
before its speech and stays 0.5 s after it, lasts 5/6 s to 7 s, keeps a 2-frame gap, and never
ends after the video. `build` then runs the passes over `Draft`s, cues whose lines, kind, speech
span and frames are still being settled:

```text
aligned.json ─▶ segment::units ─▶ segment::drafts ─▶ timing::initial ─▶ shots::snap
                                                                           │
cues.json ◀─ timing::clamp ◀─ timing::chain ◀─ sound::place ◀─ (extend, separate) ×2
```

`segment` splits each speaker's turn at sentence ends, then clauses, then pauses of 250 ms, until
each unit fits two lines of 42 characters and 6.4 s of speech; narration is italic. Two speakers
share one cue when one of them alone would be too short or too fast: as `-Line` / `-Line` when the
change was marked (`||`, or the `SPK` flag on the utterance), the gap is under 12 frames and each
fits one line with one sentence; as one cue when no change was marked and the words fit.
`timing::extend` lets a cramped cue take back the lead-out of the cue before, down to that cue's
speech and its own minimum. `line_break`
scores every break point: after punctuation and before a conjunction is good, splitting an article
from its noun or a name is bad, and a shorter top line is preferred.

`shots::cut_frames` keeps the scene changes scoring at least the job's cut score and merges
changes closer than 0.5 s into the strongest; `snap` starts a cue on a cut its speech follows
closely and ends it 2 frames before a nearby cut. `timing::extend` grows short or fast cues into
the gap, `separate` restores the 2-frame gap keeping speech covered first, `chain` closes gaps of
3 frames up to just under the lead-out, and `clamp` stops every cue at the video's end.

`sound::place` gives a sound cue its own cue where the dialogue leaves at least 0.8 s free around
it; otherwise it becomes the second line of the one-line dialogue cue it overlaps, when the text
fits; otherwise it is dropped and named in the result. A song's music cue is placed the same way
and lasts at most 7 s. When rules clash, speech covered wins, then the gap, then the minimum
duration, then the shot rules.

## Boundaries

- Depends on: `subtitle_formats::cue` (`FrameRate`, `Cue`, `CueKind`, `CueLine`, `CueTrack`),
  `job_model::outputs` (`Aligned`, `ShotChanges`, `SoundCue`).
- Used by: `crates/pipeline/src/tasks/layout.rs` (the cue step, which writes `cues.json` and the
  dropped sound cues); `crates/stages/src/qc/`, which checks cues with `FrameRules`,
  `line_break::MAX_LINE` and `segment::MAX_CPS`.
- Rules:
  - the stage runs inside the job runner, not in a worker, so it loads no model and starts no
    child process (`placement` in `crates/pipeline/src/graph/mod.rs`);
  - a whole scene builds ordered cues on whole frames
    (`a_whole_scene_builds_ordered_frame_true_cues` in `tests/build.rs`);
  - no line passes 42 characters and no cue passes two lines
    (`text_over_two_lines_does_not_fit` in `tests/line_break.rs`);
  - overlapping cues are pulled apart keeping speech first, and no cue ends after the video
    (`overlapping_cues_are_pulled_apart_keeping_speech_first`, `no_cue_ends_after_the_video` in
    `tests/timing.rs`);
  - a sound cue with no room is dropped, never squeezed in (`a_sound_with_no_room_is_dropped` in
    `tests/sound.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#9-cue-building) — the layout and timing rules
  in short.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — the full rules.
