# SRT writer

SubRip (`.srt`): numbered cues, `HH:MM:SS,mmm` times and `<i>` italics, the format the output
step installs beside the video.

## Contents

```text
crates/subtitle_formats/src/writers/srt/
├── mod.rs  `write` (the whole file) and `timestamp` (one frame as `HH:MM:SS,mmm`)
└── tests/  unit tests for timestamps, numbering, italics, an empty track and text kept as given
```

## Boundaries

- Depends on: `crate::cue::{CueTrack, FrameRate}`.
- Used by: `crates/pipeline/src/tasks/layout.rs`, which writes the track's SRT text and hands it
  to the output stage.
- Rules:
  - cues are numbered from 1, each followed by a blank line, with LF line ends
    (`cues_are_numbered_from_one_with_blank_lines_between` in `tests/srt.rs`);
  - a time is the cue's frame rounded to the nearest millisecond
    (`timestamps_use_hours_minutes_seconds_and_rounded_milliseconds`);
  - each italic line is wrapped in `<i>` on its own (`italic_lines_are_wrapped_one_by_one`);
  - the text is written as given (`the_writer_keeps_text_as_given`), and an empty track is an
    empty file (`an_empty_track_is_an_empty_file`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — SRT as
  the default output.
