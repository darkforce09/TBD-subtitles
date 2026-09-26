# WebVTT writer

WebVTT (`.vtt`): the `WEBVTT` header, `HH:MM:SS.mmm` times and `<i>` italics, one of the output
formats the owner can choose in the settings.

## Contents

```text
crates/subtitle_formats/src/writers/vtt/
├── mod.rs  `write` (the whole file) and `timestamp` (one frame as `HH:MM:SS.mmm`)
└── tests/  unit tests for timestamps, the header, italics, escaping and an empty track
```

## Boundaries

- Depends on: `crate::cue::{CueTrack, FrameRate}`.
- Used by: `crates/pipeline/src/tasks/layout.rs`, the output step, when the job's output format is
  WebVTT.
- Rules:
  - the file starts with `WEBVTT` and a blank line; cues carry no numbers
    (`the_file_starts_with_the_header_and_cues_are_unnumbered` in `tests/vtt.rs`);
  - a time is the cue's frame rounded to the nearest millisecond
    (`timestamps_use_a_dot_before_the_milliseconds`);
  - `&`, `<` and `>` are written as references so the text shows as given
    (`reserved_characters_are_written_as_references`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — the
  output formats.
