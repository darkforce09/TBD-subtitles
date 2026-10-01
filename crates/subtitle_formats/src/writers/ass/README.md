# ASS writer

[Advanced SubStation Alpha](/documentation/glossary.md#ass) (`.ass`): a script header, one
bottom-centred dialogue style and `H:MM:SS.cc` times, one of the output formats the owner can
choose in the settings and the format of the localized video's subtitle file. The positioned
[sign](/documentation/glossary.md#sign) events are written by the typesetting step in
`crates/stages/src/onscreen_text/typeset.rs`, not here.

## Contents

```text
crates/subtitle_formats/src/writers/ass/
├── mod.rs        `write`, `write_with` (cues moved clear of obstacles), `timestamp`, `PLAY_RES`
├── placement.rs  `Obstacle` and the band each cue is drawn in: bottom, or top over writing
└── tests/        unit tests for timestamps, dialogue lines, escaping, an empty track, placement
```

## How it works

The script's canvas is 1920 by 1080 (`PLAY_RES`). The `Default` style is Arial 64, white with a
3-pixel black outline and a 1-pixel shadow, bottom centre (alignment 2), 54 pixels above the
bottom edge, with wrapping off (`WrapStyle: 2`), since the cue stage already broke the lines. Each
cue is one `Dialogue` line: its lines joined by `\N`, an italic line wrapped in `{\i1}` and
`{\i0}`.

`write_with` takes obstacles: writing in the picture, each a rectangle on the canvas with the
seconds it is shown. For each cue it estimates the cue's box in the `Default` style: one line
height (1.25 × 64 pixels) per line, stacked up from 54 pixels above the bottom edge, as wide as the
longest line at 0.52 × 64 pixels a character, centred and at most the width between the 120-pixel
margins. When that box meets an obstacle shown during the cue, the same box 54 pixels below the
top edge is measured too, and the cue moves to the top with a leading `{\an8}` if the top box
meets less of the writing; the bottom wins a tie. `write` is `write_with` without obstacles, so
it never moves a cue.

## Public surface

- `write`, `write_with`, `timestamp`, `Obstacle` and `PLAY_RES`, for the output step in
  `crates/pipeline/src/tasks/layout.rs`.

## Boundaries

- Depends on: `crate::cue::{Cue, CueTrack, FrameRate}`.
- Used by: `crates/pipeline/src/tasks/layout.rs`, the output step, when the job's output format is
  ASS and for the localized video's subtitle file; `tools/visual_validation/src/pilot.rs`, for
  its preview script.
- Rules:
  - a time is the cue's frame rounded to the nearest centisecond
    (`timestamps_use_hours_and_rounded_centiseconds` in `tests/ass.rs`);
  - every cue is one `Dialogue` line in the `Default` style
    (`every_cue_is_one_dialogue_line_in_the_default_style`);
  - `{`, `}` and a backslash before `n`, `N` or `h` show as written
    (`braces_and_backslash_sequences_show_as_written`);
  - a cue over writing shown with it moves to the top
    (`a_cue_over_lettered_writing_moves_to_the_top` in `tests/placement.rs`), writing at another
    time or away from the bottom band leaves it where it is
    (`writing_at_another_time_or_at_the_top_leaves_the_cue_at_the_bottom`), with both bands
    covered it takes the one covered less
    (`with_both_bands_blocked_the_cue_takes_the_side_that_covers_less`), and `write` never moves a
    cue (`the_normal_file_has_no_placement_override`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — the
  output formats and the placement over on-screen text.
