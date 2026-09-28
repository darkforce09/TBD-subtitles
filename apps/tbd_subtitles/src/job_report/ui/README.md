# Job report view

The Overview of a finished job, drawn from a borrowed `JobReport`; it returns events and changes
nothing.

## Contents

```text
apps/tbd_subtitles/src/job_report/ui/
├── file_card.rs        the verdict, Fix It's notes, the correction note, problems, the Fix It row, buttons
├── fix_result_card.rs  Fix It's result: what Claude changed, cleared and left, examples, See Changes
├── lines_card.rs       the lines worth a listen, the checked bar, Check Lines, a row per group
├── mod.rs              the module list and the entry point
├── overview.rs         `OverviewView` and `overview_ui`: the cards in order; the head both cards share
└── report_details.rs   the Details and Step times disclosures, and Open Full Report
```

## How it works

`overview_ui` draws four cards in the application's column, 16 px apart. The file card's head is
a 28 px captions mark (green, or orange when there are problems), "Subtitles saved next to the
video" with a line under it, and on the right the pill "Passes the quality check" or "Needs
attention". Once Fix It has answered lines, and while it is not under way on the video, its
result follows the head on green: a seal, "Fixed by Claude Opus", "17 lines changed · 21 were
already right", each problem it cleared with a check in green ("Speech with no subtitle"), each
problem left in orange ("Claude could not fix: …") with its fix and its button, the first two
changes in plain words (“Heaven dish” → “Cavendish”, Added “Uh,”, Removed “So”) and "and 15
more", what the owner
did with the changes ("You kept 3 · undid 1"), the changes turned down and the lines that kept the
owner's correction, and See Changes, which opens Check Lines on Changed by Claude (on the lines to
check once the owner kept or undid every change). Just after Fix It finished, the seal grows from
0.6 of its size in 0.35 s and the green fades from strong to its usual tint in 2 s. Each problem
follows on the recessed well, unless the result lists it: an orange warning mark, its title in
semibold, its fix in grey and its small button (Try Again with a turning arrow, Show Nearby Lines,
Show Lines); while Fix It runs on the video, a blue note says "Fixing with Claude Opus · reading
the whole video (1 of 4)…" with the calls done and Stop (then "Stopping…"), and while its
correction run waits or runs, "Fixing with Claude Opus · updating the subtitles (4 of 4)…"; while
any other correction run of the video waits or runs, one says "Updating subtitles with 2
corrections…". When Fix It has findings to ask about, a row on the well follows the problems: a
wand, "Fix It with Claude Opus", what it does, and Fix It, primary when the job needs attention and
off with why while it cannot run. Then the path on the well as its folder and file
("…/one_pace/[Muhn Pace] Dressrosa 15.srt", the whole path on hover), and Open in
Player, Show in Folder and Copy Path (which puts the path on the clipboard). The lines card says
"38 lines worth a listen" (or "All 38 lines checked", with "Claude checked 36 · you checked 2"
when Claude checked some) under an ear mark, a green bar of the lines the owner and Claude checked
at most 320 px wide with "12 of 38 checked", and the blue 32 px Check Lines (Show Checked Lines
once none is left); under a line, one row per group with lines: its orange mark, its title
in semibold over its explanation, its count and a chevron, the whole row a button; the Changed by
Claude row comes first, with a wand. A job with no line worth a listen shows "No lines need a listen" instead. Details and Step times are `core::ui::disclosure`
rows, closed at first and kept per job in egui's memory: Details lists the subtitles, the share
easy to read, the unsure lines, the words no engine heard, the timing offset, the speech and
voice with no subtitle, the words timed by the aligner and the corrections the owner made, a 220 px key
column beside the values; Step times shows the six stages on the well with their times, each
step indented under its stage with its time, peak RAM and peak VRAM right-aligned ("—" when not
measured), and Open Full Report.

## Boundaries

- Depends on: `crate::job_report::{events, models}`; `crate::core::{format, steps, ui}`;
  `eframe`; `job_model` for the steps and the timing sources.
- Used by: `crate::application::feature_views`.
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); the Overview renders as tested in
  `apps/tbd_subtitles/src/application/tests/rendering_report.rs`, and Fix It's notes and result
  as tested in `apps/tbd_subtitles/src/application/tests/rendering_fix_it.rs`.
