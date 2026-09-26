# ASS writer

[Advanced SubStation Alpha](/documentation/glossary.md#ass) (`.ass`): a script header, one
bottom-centred dialogue style and `H:MM:SS.cc` times, one of the output formats the owner can
choose in the settings. Positioned [sign](/documentation/glossary.md#sign) styles come with the
Japanese on-screen text feature.

## Contents

```text
crates/subtitle_formats/src/writers/ass/
├── mod.rs  `write` (the whole script) and `timestamp` (one frame as `H:MM:SS.cc`)
└── tests/  unit tests for timestamps, the dialogue lines, escaping and an empty track
```

## How it works

The script's canvas is 1920 by 1080. The `Default` style is Arial 64, white with a 3-pixel black
outline and a 1-pixel shadow, bottom centre (alignment 2), 54 pixels above the bottom edge, with
wrapping off (`WrapStyle: 2`), since the cue stage already broke the lines. Each cue is one
`Dialogue` line: its lines joined by `\N`, an italic line wrapped in `{\i1}` and `{\i0}`.

## Boundaries

- Depends on: `crate::cue::{CueTrack, FrameRate}`.
- Used by: `crates/pipeline/src/tasks/layout.rs`, the output step, when the job's output format is
  ASS.
- Rules:
  - a time is the cue's frame rounded to the nearest centisecond
    (`timestamps_use_hours_and_rounded_centiseconds` in `tests/ass.rs`);
  - every cue is one `Dialogue` line in the `Default` style
    (`every_cue_is_one_dialogue_line_in_the_default_style`);
  - `{`, `}` and a backslash before `n`, `N` or `h` show as written
    (`braces_and_backslash_sequences_show_as_written`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — the
  output formats.
