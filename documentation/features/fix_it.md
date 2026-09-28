**Status:** live

# Fix It

One button on a finished job's Overview that has a stronger `claude` model (Opus by default) fix
the lines the quality check flagged, so the owner does not have to open each one. The model reads
the whole video first to work out its context by itself, fixes the lines one kind of problem at a
time, and a judge keeps a change only when it reads right against the line before it. Every word
it leaves in a line was heard by a speech engine, and every change waits in Check Lines for the
owner to keep or undo.

## Where it lives

- Code: the passes are `crates/stages/src/fix_it/`; the run on a finished job, with its cache and
  the merge into the corrections, is `crates/pipeline/src/fix_it/`; the window's thread is
  `apps/tbd_subtitles/src/job_report/services/fix_it.rs`, its button and note
  `apps/tbd_subtitles/src/job_report/ui/file_card.rs`, and its actions
  `apps/tbd_subtitles/src/application/actions/fix_it.rs`; Keep Change and Undo Change are in
  `apps/tbd_subtitles/src/line_review/`.
- Entry: Fix It on the Overview of a finished job, or `tbd-subtitles fix <VIDEO>` without a
  window (`apps/tbd_subtitles/src/cli/fix_command.rs`), which runs the correction run after it.
- Related: the [desktop GUI](/documentation/features/gui.md) it sits in, and the
  [pipeline](/documentation/architecture/pipeline.md#fix-it) whose findings it fixes.

## Behaviour

### What it asks about

Everything the quality check flags, but the aligner's offset (no line to fix) and failed
language-model calls (Try Again runs those again):

| Family | Findings | What the model may do |
|---|---|---|
| Words | unsure lines, words no engine heard, heard words replaced, and lines the brief finds out of place | choose between heard readings, put back a heard word, spell a glossary name |
| Timing and layout | speech with no subtitle (the lines within 0.5 s of it), loosely timed lines, every layout rule | put back a heard word at a line's edge, take out words only one engine heard that the aligner could not place, fix a speaker flag, drop noise; most need no word change, since every returned line is timed again |
| Reading speed | subtitles over 20 characters per second | only take words out: pure filler, a stutter or a repeat that adds nothing |

A line the owner saved, kept or took from Fix It is never asked about. A Fix It change the owner
has not checked is not asked about its words again.

### The three passes

1. **Reading the whole video** (1 of 3). The owner types nothing: the model reads the video's
   file and folder names (`[Muhn Pace] Dressrosa 12` in `one_pace`), the glossary and every line,
   and uses what it knows of the series to name the show, the episode and the cast, sum up the
   scenes, list speech habits that are meant (a stutter, a catchphrase) so they are not fixed,
   and name up to 30 lines that do not fit the conversation. A video of more than 800 lines is
   read in parts.
2. **Fixing** (2 of 3): words, then timing and layout, then reading speed, in batches of 25 lines,
   several calls at once. Each line shows the model its problems, where it sits and the gaps
   around it, what each engine heard, the line now, which words the aligner could not place, and
   three lines on each side. A guard holds every answer: no word no engine heard, no empty line
   unless dropped, no sheet notation, a reason given, and for reading speed words taken out only.
3. **Checking each change** (3 of 3): a judge sees each changed line before and after, why it
   changed and the heard words it now leaves out, and accepts it only when every word was heard,
   it reads right in the scene and for the character, and each word taken out is truly filler, a
   stutter, a stray from another voice or a mishearing. Restyling and anything in doubt is turned
   down.

A 27-minute episode takes about ten calls.

### In the window

```text
 Overview, file card
  Fix It with Claude Opus   "Claude Opus reads the whole video, fixes the flagged lines and
                             checks each change. You can keep or undo every change in Check
                             Lines."                                               [Fix It]
        │  (primary when the job needs attention; off with why while it cannot run)
        ▼
  Fixing with Claude Opus · reading the whole video (1 of 3)…                        [Stop]
        │
        ▼  kept changes go into review.json ──▶ a correction run times them
  toast: "Claude Opus changed 14 lines in Dressrosa 12. Updating the subtitles."
  lines card: "Changed by Claude" group first ──▶ Check Lines
        each line: "Claude Opus changed this line" ─ what the app had and why
        [Keep Change] (Ctrl+Enter)  makes Claude's text the owner's
        [Undo Change]               puts the app's reading back as the owner's
```

- The Fix It row shows when Fix It has a finding to ask about. It is off while a run of the video
  runs or its correction run waits ("Wait until this video's subtitles are updated."), and while
  another video is being fixed ("Fix It is fixing another video; it fixes one at a time.").
- While it runs, no run of the video starts (a correction run the owner queues waits for it), and
  the row cannot be removed, tried again or run again. Check Lines stays open to the owner; a line
  the owner saves meanwhile keeps the owner's correction.
- Stop ends the run at once: "Fix It stopped. Nothing was changed; Fix It again picks up where it
  stopped." Every answered call is kept, so the next Fix It asks only what is left.
- When it ends, the toast says how many lines changed, how many kept the owner's own correction,
  and how many calls failed; "Claude Opus found nothing to change in Dressrosa 12." when none did.
- A changed line is in the Changed by Claude group, with a wand and the "Claude" chip, and stays
  in To Check until the owner keeps or undoes it. Its quality-check words are checked again on
  Claude's text, and the report counts it apart: "Lines Fix It changed, not checked yet: 14".

## Data

- Settings: the Fix It model is `fix_model` in the `[language_model]` table of `settings.toml`,
  `opus` by default, chosen in Settings, Engines, "Fix It model" (Sonnet, Opus, Fable or Haiku);
  the run's own model stays `model`, `sonnet` by default. Fix It runs as many calls at once as
  `processes`.
- `review.json`: each kept change as a correction whose `chosen` is `fix_it` with the model and
  the reason; Keep Change makes it `kept_fix_it`, the owner's. The window and Fix It change the
  file only under its lock, `review.json.lock`.
- `fix.json`: the last run: the brief, and for each line asked about its problems, each change
  that passed the guard, the proposals refused, the heard words left out, the verdict and whether
  it was applied; the calls, tokens and cost.
- `fix/calls/`: each answered call of a run not finished yet, removed once the run writes its
  corrections.

## Design

- Fix It runs on a thread of the window, not as a queued job: it writes corrections, not steps,
  and the existing correction run times them. Closing the window ends a run; its answered calls
  stay for the next one.
- The model sees each line's start, its length and the gaps around it, never a word's time.
- The `claude` calls run with no tools, as for adjudication; the context is the model's own
  knowledge and the job's own files.

## Open work

None.

## Decisions

- Fix It fixes the flagged lines in three passes, a guard and a judge holding every change to what
  the engines heard, and the owner keeping or undoing each
  ([Fix It](/documentation/decisions/desktop_gui.md#2026-09-28--fix-it-a-stronger-model-fixes-the-flagged-lines-in-three-passes)).
