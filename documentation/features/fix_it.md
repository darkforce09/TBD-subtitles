**Status:** live

# Fix It

One button on a finished job's Overview that has a stronger `claude` model (Opus by default) fix
the lines the quality check flagged, so the owner does not have to open each one. The model reads
the whole video first to work out its context by itself, fixes the lines one kind of problem at a
time, and a judge keeps a change only when it reads right against the line before it. Every word
it leaves in a line was heard by a speech engine, and every change waits in Check Lines for the
owner to keep or undo. Once the subtitles are updated, the Overview shows what it did.

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
has not checked is not asked about its words again. A finding an earlier Fix It answered (its
line, about the same check) is not asked about again, nor a line the brief names that an earlier
run answered; a new problem on the same line is. Speech with no subtitle is asked about every
time. A line whose every call failed, or whose change the judge gave no verdict on, is not
answered, so the next Fix It asks about it again.

### Four steps

Fix It works in three passes of its model, then updates the subtitles; its progress counts all
four.

1. **Reading the whole video** (1 of 4). The owner types nothing: the model reads the video's
   file and folder names (`[Muhn Pace] Dressrosa 12` in `one_pace`), the glossary and every line,
   and uses what it knows of the series to name the show, the episode and the cast, sum up the
   scenes, list speech habits that are meant (a stutter, a catchphrase) so they are not fixed,
   and name up to 30 lines that do not fit the conversation. A video of more than 800 lines is
   read in parts.
2. **Fixing** (2 of 4): words, then timing and layout, then reading speed, in batches of 25 lines,
   several calls at once. Each line shows the model its problems, where it sits and the gaps
   around it, what each engine heard, the line now, which words the aligner could not place, and
   three lines on each side. A guard holds every answer: no word no engine heard, no empty line
   unless dropped, no sheet notation, a reason given, and for reading speed words taken out only.
3. **Checking each change** (3 of 4): a judge sees each changed line before and after, why it
   changed and the heard words it now leaves out, and accepts it only when every word was heard,
   it reads right in the scene and for the character, and each word taken out is truly filler, a
   stutter, a stray from another voice or a mishearing. Restyling and anything in doubt is turned
   down.
4. **Updating the subtitles** (4 of 4): the kept changes go into `review.json`, and a
   [correction run](/documentation/glossary.md#correction-run) times them and rebuilds the cues,
   the quality check and the subtitle file. The Overview's note says "Fixing with Claude Opus ·
   updating the subtitles (4 of 4)…". When Claude changed nothing, there is no step 4: Fix It
   finishes at once, with no correction run.

A 27-minute episode takes about ten calls.

### In the window

```text
 Overview, file card
  Fix It with Claude Opus   "Claude Opus reads the whole video, fixes the flagged lines and
                             checks each change. You can keep or undo every change in Check
                             Lines."                                               [Fix It]
        │  (primary when the job needs attention; off with why while it cannot run)
        ▼
  Fixing with Claude Opus · reading the whole video (1 of 4)…                        [Stop]
        │  sidebar: "Fixing with Claude · 1 of 4", with the working spinner
        ▼  kept changes go into review.json ──▶ a correction run times them
  Fixing with Claude Opus · updating the subtitles (4 of 4)…
        │
        ▼  the correction run ends
  Result card "Fixed by Claude Opus" · toast with [See Changes] · notification when away
  lines card: "Changed by Claude" group first ──▶ Check Lines
        each line: "Claude Opus changed this line" ─ what the app had and why
        [Keep Change] (Ctrl+Enter)  makes Claude's text the owner's
        [Undo Change]               puts the app's reading back as the owner's
```

- The Fix It row shows while a flagged finding is one Claude has not answered. It hides once
  every flagged finding is answered, and shows again when the quality check finds one Claude has
  not seen. It is off while a run of the video runs or its correction run waits ("Wait until this
  video's subtitles are updated."), and while another video is being fixed ("Fix It is fixing
  another video; it fixes one at a time.").
- While it runs, no run of the video starts (a correction run the owner queues waits for it), and
  the row cannot be removed, tried again or run again. Check Lines stays open to the owner; a line
  the owner saves meanwhile keeps the owner's correction.
- Stop ends the run at once: "Fix It stopped. Nothing was changed; Fix It again picks up where it
  stopped." Every answered call is kept, so the next Fix It asks only what is left.
- A changed line is in the Changed by Claude group, with a wand and the "Claude" chip, until the
  owner keeps or undoes it. Its quality-check words are checked again on Claude's text, and the
  report counts it apart: "Lines Fix It changed, not checked yet: 14".

### The finish

Fix It finishes when the correction run ends, so what it says is already in the subtitle file.
It finishes in four ways at once:

- **The Result card** on the Overview, in green ([below](#the-result-card)).
- **A toast** for 8 seconds, with See Changes: "Dressrosa 12 is fixed: Claude changed 17 lines.
  The subtitles are ready.", or "Dressrosa 12 is fixed: Claude changed 17 lines; 1 problem is
  left for you." while a problem is left.
- **A desktop notification**, through the desktop portal, with a flashing taskbar entry, only
  when the window is not in front or is minimized: the title "Dressrosa 12 is fixed" and the body
  "Claude changed 17 lines. The subtitles are ready."
- **The card's seal pops in:** it scales up over 0.35 s, and its green fades to its usual shade
  over 2 s.

When Claude changed nothing, Fix It finishes at once, with no correction run, and the toast says
"Claude checked Dressrosa 12: every line was already right."

### The Result card

```text
 Overview, file card
  Subtitles saved next to the video                       (✓ Passes the quality check)
  ┌ (seal) Fixed by Claude Opus ─────────────────────────────────────────────────────┐
  │ 17 lines changed · 21 were already right                                         │
  │ ✓ <each problem it cleared>                                                      │
  │ Claude could not fix: <each problem left>                     [its usual button] │
  │ “Heaven dish” → “Cavendish”                                                      │
  │ Added “Uh,”                                                                      │
  │ and 15 more                                                                      │
  │ You kept 3 · undid 1                                               [See Changes] │
  └──────────────────────────────────────────────────────────────────────────────────┘
```

- **Where:** in the file card, right under its heading. It shows while the video's `fix.json`
  counts (the video has not been adjudicated again since Fix It ran), never while Fix It runs or
  the subtitles are updating.
- **Title:** "Fixed by Claude" and the Fix It model's name: "Fixed by Claude Opus".
- **Counts:** the lines changed and the lines Claude answered with no change: "17 lines changed ·
  21 were already right".
- **Cleared:** each problem from before the first run that the quality check no longer finds,
  with a green check. A `fix.json` written before Fix It kept those problems shows no cleared
  list.
- **Left:** each problem still there, as "Claude could not fix: …", with its usual button (the
  [whole-video problems](/documentation/features/gui.md#whole-video-problems)).
- **Examples:** two changes in plain words: a replaced word as “Heaven dish” → “Cavendish”, an
  added word as Added “Uh,”, a removed word as Removed “So”; then "and 15 more".
- **The owner's part:** "You kept 3 · undid 1" once the owner kept or undid changes in Check
  Lines.
- **See Changes**, on the card and in the toast, opens Check Lines on the Changed by Claude group,
  or on the lines to check once the owner has kept or undone every change.

### Lines Claude checked

- A line Claude changed or answered counts as checked. With none left to check, the lines card
  reads "All 38 lines checked" with "Claude checked 36 · you checked 2".
- The sidebar row reads "Fixing with Claude · 2 of 4", with the working spinner, while Fix It
  runs, and "Subtitles ready · fixed by Claude" once the job passes with nothing left to check.
- Details' "Corrections you made" counts the owner's corrections only, not Claude's.

## Data

- Settings: the Fix It model is `fix_model` in the `[language_model]` table of `settings.toml`,
  `opus` by default, chosen in Settings, Engines, "Fix It model" (Sonnet, Opus, Fable or Haiku);
  the run's own model stays `model`, `sonnet` by default. Fix It runs as many calls at once as
  `processes`.
- `review.json`: each kept change as a correction whose `chosen` is `fix_it` with the model and
  the reason; Keep Change makes it `kept_fix_it`, the owner's. The window and Fix It change the
  file only under its lock, `review.json.lock`.
- `fix.json`: every Fix It run of the video since it was last adjudicated: the last run's brief;
  for each line asked about, by id, its problems and the checks behind them, each change that
  passed the guard, the proposals refused, the heard words left out, the verdict and whether it
  was applied; the calls, tokens and cost of all the runs; the quality check's problems before
  the first run (the count per check, where the first speech with no subtitle starts, the share
  of cues within 20 characters per second and the number of cues); and the fingerprint of the
  re-adjudication the runs read. Each run keeps the earlier runs' lines it did not ask about
  again and replaces the ones it did. Once the video is adjudicated again (the fingerprint
  differs), the record counts for nothing: the next run asks about everything and starts a new
  one. The Result card reads it while it counts.
- `fix/calls/`: each answered call of a run not finished yet, removed once the run writes its
  corrections.

## Design

- Fix It runs on a thread of the window, not as a queued job: it writes corrections, not steps,
  and the existing correction run times them. Closing the window ends a run; its answered calls
  stay for the next one.
- The model sees each line's start, its length and the gaps around it, never a word's time.
- The `claude` calls run with no tools, as for adjudication; the context is the model's own
  knowledge and the job's own files.
- Every run's `claude` calls pass one shared gate, so many videos fixed at once never start more
  calls than its cap; the calls go in the order the runs started, an answer kept from an earlier
  run skips the gate, and a call the provider answers as busy (rate limit, overloaded) is asked
  again after 30, 60 and 120 s without holding a slot while it waits.
- The finish waits for the correction run, so the card, the toast and the notification speak of
  subtitles already rebuilt. The notification and the flashing taskbar entry come only when the
  window is away, since the card says it all when the owner is looking.

## Open work

None.

## Decisions

- Fix It fixes the flagged lines in three passes, a guard and a judge holding every change to what
  the engines heard, and the owner keeping or undoing each
  ([Fix It](/documentation/decisions/desktop_gui.md#2026-09-28--fix-it-a-stronger-model-fixes-the-flagged-lines-in-three-passes)).
- Fix It finishes visibly: its progress runs through the correction run, a Result card and a toast
  say what it did, a notification comes when the window is away, and the lines it answered count
  as checked
  ([Fix It finishes visibly](/documentation/decisions/desktop_gui.md#2026-09-28--fix-it-finishes-visibly)).
