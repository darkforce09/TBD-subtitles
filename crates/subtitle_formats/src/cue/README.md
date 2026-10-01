# Cue model

The [cue](/documentation/glossary.md#cue) model: a video's frame rate and its cues, each timed in
whole frames with one or two lines, italics, and a kind for dialogue,
[sound cues](/documentation/glossary.md#sound-cue) and music.

## Contents

```text
crates/subtitle_formats/src/cue/
├── mod.rs  `FrameRate`, `CueKind`, `CueLine`, `Cue` and `CueTrack`
└── tests/  unit tests for frame conversion, reading speed and the JSON and rkyv round trips
```

## How it works

`FrameRate` is a fraction such as 24/1 or 24000/1001, refused when either part is zero. It gives
the time a frame starts in seconds (`seconds`) and in whole milliseconds rounded to the nearest
(`millis`), and turns seconds into a frame boundary with `frame_floor`, `frame_ceil` or
`frame_round`. A `Cue` is shown from frame `start` up to, not including, frame `end`; it counts
its `frames`, its characters over all lines (`chars`, line breaks not counted) and its reading
speed (`cps`). A `CueLine` is plain or italic text. `CueKind` is `dialogue` (spoken lines, perhaps
with a sound line added), `sound` or `music`. A `CueTrack` holds the frame rate and the cues in
time order, serialises to JSON (as `dump` prints it), and archives with rkyv for the job
database, where it is `outputs/cues`.

## Boundaries

- Depends on: `serde` and `rkyv` for the derives.
- Used by: `crates/subtitle_formats/src/writers/srt/`; `crates/stages/src/cues/`, which builds
  the track; `crates/stages/src/qc/`, which checks it; `crates/pipeline/src/tasks/layout.rs`,
  which stores and writes it.
- Rules:
  - a zero frame rate is refused (`a_zero_rate_is_refused` in `tests/cue.rs`);
  - film and NTSC frames round to the nearest millisecond, and seconds snap to frame boundaries
    (`film_frames_round_to_milliseconds`, `ntsc_frames_round_to_milliseconds`,
    `seconds_snap_to_frame_boundaries`);
  - reading speed counts every character but line breaks
    (`reading_speed_counts_every_character_but_line_breaks`);
  - a track round-trips through JSON and through rkyv (`a_track_round_trips_through_json`,
    `a_track_round_trips_through_rkyv`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#text-and-layout) — lines,
  speakers, italics and sound cues.
