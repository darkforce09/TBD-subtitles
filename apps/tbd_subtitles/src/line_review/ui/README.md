# Line review view

Check Lines, drawn from a borrowed `ReviewView`; it returns events and changes nothing.

## Contents

```text
apps/tbd_subtitles/src/line_review/ui/
├── clip_view.rs    the picture (still frame, then the clip's), the timeline with its playhead, Play
├── heard_list.rs   what was heard: the language model's pick and each engine's, with Use or In use
├── line_editor.rs  the open line: clip, now, heard, text, flags, footer; or all done
├── line_list.rs    To Check / Checked / All, the search, the group pill, and a row per line
├── line_status.rs  the open line's head with its run's chip, and why it is worth a listen
├── mod.rs          the module list and what the application uses
└── review_view.rs  `ReviewView`, `Playing` and `review_view_ui`: the list beside the editor
```

## How it works

`review_view_ui` puts the list in a 330 px left panel and the editor over the grouped grey. The
list's tools are the three lists across the top with their counts, the search field ("Search
text or time"), and, while the list is narrowed to a group, "Showing" with the group's orange
pill whose ✕ shows every group again. Each row has a mark (an orange dot to check, a blue dot
while edited, a wand for a Fix It change, a green check once checked), the line's time to the tenth of a second, its text as
it stands now cut after two lines, and chips: "Edited, not saved", "Looks right" or "Corrected",
else one per group, Fix It's "Claude" first in the accent colour. The row open is tinted with a blue bar and scrolled into view when it
changes.

The editor shows the line's time and id with its length, and on the right the status chip of its
correction run (Saved, Updating subtitles…, Subtitles updated, Subtitles not updated), from
`line_status.rs`; for a Fix It change a blue box naming the model with what the app had and why,
then the rest; else an orange box per group saying why, or a green one saying the owner kept or
corrected it (or kept Claude's change); the clip; "In the
subtitles now"; what was heard; the text box with "Type || where a second speaker starts." and
"Esc leaves the text box."; and the four flags as cards with a switch and what the subtitles do
with it (a new speaker is never merged into the line before and shares a subtitle only with
dashes; the narrator is in italics and never in a two-speaker subtitle; a song lyric is left out
of the dialogue and its song can get a sound cue; a dropped line is left out). The footer holds
Previous and Next, off at the list's ends, then "Edited, not saved" with Discard Edit and Save
Correction (Ctrl+S), or Keep Change (Ctrl+Enter) and Undo Change for a Fix It change, or Take
Back, or Looks Right (Ctrl+Enter), whose hover text says the line is timed again and its
warnings clear. With no line shown it says every line is checked and offers
Show Checked Lines, or asks for another filter.

The clip view draws the picture at most 480 px wide in a 16:9 box on the video black, with the
time at its lower right (the line's start, where the still frame is from, until the clip plays); the plain box while the still frame decodes, the film mark only when
there is no picture. The timeline under it hatches the 0.75 s pads, tints the line's span with
its length, and paints the red playhead at the line's start, or where the playing clip is. Under
it stand the clip's start, "line … – …" and its end, then Play and Voices Only with "Space plays"
or Stop with what is playing. The picture's texture is kept in egui's memory and uploaded only
when a frame with a new serial arrives.

## Boundaries

- Depends on: `crate::line_review::{events, models, services::{line_filter, review_editing,
  clip_player}}`; `crate::job_report::models::finding_group`; `crate::core::{format, ui}`;
  `media_io::preview::Clip`; `eframe`.
- Used by: `crate::application::feature_views`.
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); nothing is saved while a full run of the
  job's video runs (the header of `line_editor.rs`); the strings and states are checked in
  `apps/tbd_subtitles/src/application/tests/rendering_review.rs`.
