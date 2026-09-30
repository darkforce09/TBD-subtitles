**Status:** live

# Desktop GUI

The window the owner uses to make subtitles without a terminal: queue videos, run them one at a
time, watch each job's progress and time left, read whether its subtitles pass the quality check,
and fix dialogue and visible Japanese writing. It keeps the owner's approved macOS-like design:
a toolbar, a video sidebar, Overview, Check Lines and Check Text, and a separate Settings window.
The accepted Dressrosa 12–48 batch completes M2. The integrated
[visual translation feature](/documentation/features/japanese_onscreen_text.md) has acceptance underway.

## Where it lives

- Code: `apps/tbd_subtitles/`, built with eframe (egui) on the glow renderer. The shell (state,
  frame, actions, shortcuts, the Settings and log windows) is
  `apps/tbd_subtitles/src/application/`; the shared look and widgets are
  `apps/tbd_subtitles/src/core/ui/`, the log buffer `apps/tbd_subtitles/src/core/log_buffer/`;
  one feature folder each holds the
  queue (`apps/tbd_subtitles/src/job_queue/`), the report (`apps/tbd_subtitles/src/job_report/`),
  the line and text reviews (`apps/tbd_subtitles/src/line_review/`, `apps/tbd_subtitles/src/text_review/`), the log window
  (`apps/tbd_subtitles/src/log_console/`) and the settings (`apps/tbd_subtitles/src/settings/`).
- Entry: `tbd-subtitles gui [VIDEO]...`, or the binary with no subcommand, with or without videos.
  It runs on the host, opened from the container with
  `distrobox-host-exec target/debug/tbd-subtitles gui`
  ([development environment](/documentation/runbooks/development_environment.md#steps)), under X11
  (XWayland on the owner's KDE Wayland desktop). [Automation](/documentation/features/automation.md)
  feeds the same queue: Dolphin's entry, watch folders, and later launches.
- Related: the [pipeline](/documentation/architecture/pipeline.md) each job runs, and the
  [subtitle style rules](/documentation/architecture/subtitle_style_rules.md) the quality check
  holds the subtitles to.

## Behaviour

### The window

It opens at 1280 by 800 and can shrink to 1100 by 700, the least that fits the sidebar, the line
list and the line editor side by side.

```text
┌───────────────────────────────────────────────────────────────────────────────────────────┐
│ [Add Videos…] [Add Folder…]      [Start Queue]  Download the models first    [log] [gear] │
├───────────────────────────────────────────────────────────────────────────────────────────┤
│ ! 5 models and 2 runtime libraries are missing (10.7 GiB)          [Details…] [Download]  │
├───────────────────────────┬───────────────────────────────────────────────────────────────┤
│ NOW                     1 │ Dressrosa 11            [Overview | Check Lines | Check Text] │
│  > Dressrosa 12           │ 25:59 video · finished in 4 min 37 s                          │
│    Hearing the speech     │ ┌───────────────────────────────────────────────────────────┐ │
│ UP NEXT                 2 │ │ the selected job's cards, in a column at most 800 px wide │ │
│  o Dressrosa 13           │ │  running:  progress card over nine stage groups          │ │
│    Waiting · next in line │ │  waiting:  place in line, Run Next, Remove from List      │ │
│  o Dressrosa 14           │ │  failed or cancelled: what happened, Try Again            │ │
│ DONE                    1 │ │  finished: Overview (file card, lines card, Details,      │ │
│  v Dressrosa 11           │ │            Step times), Check Lines or Check Text        │ │
│    Subtitles ready ·      │ └───────────────────────────────────────────────────────────┘ │
│    38 to check            │                                                               │
│ sidebar, 272 px           │ detail pane                                                   │
└───────────────────────────┴───────────────────────────────────────────────────────────────┘
                     toasts, bottom centre, over the panes:  "Removed Dressrosa 13  [Undo]"
```

- **Toolbar** (52 px): Add Videos… and Add Folder… open the desktop's chooser (files, or a folder
  for its videos without subtitles, never a `*.localized.mkv` the app wrote); asked again while one is open, a toast says "A file chooser
  is already open." The queue's one button is Start Queue, Pause After This
  Video or Resume Queue; a disabled Start Queue says why beside it ("Download the models first",
  "Nothing is waiting"). It counts full runs only. The log button (a terminal window) opens the
  [log window](#the-log-window); the gear opens Settings.
- **Banner:** under the toolbar while models are missing. It says what is missing ("5 models and
  2 runtime libraries are missing (10.7 GiB)"), with Details… (the Models tab) and Download; while
  they download it shows the bytes on disk, the item now and a bar, with Stop (a stopped file
  resumes next time); then for a moment "All models are on disk." No job starts before then.
- **Sidebar:** one row per video in the sections Now (running), Up Next (waiting) and Done
  (finished, failed or cancelled, newest first), each heading with its count and folding away on a
  click. While finished videos have lines to fix, the Done heading holds a small Fix All button with
  a wand just left of its count ("Fix It on the 12 finished videos with lines to fix" on hover),
  which starts [Fix It](/documentation/features/fix_it.md) on each, oldest finished first; a click
  on it never folds the section. A row has a status mark, the video's name and one line: "Waiting ·
  2nd in line", "Failed at Hear the speech", "Subtitles ready · 38 to check" (the count in orange),
  "Needs attention · 1 problem", "Updating subtitles · 1 correction" while a
  [correction run](/documentation/glossary.md#correction-run) of its video runs, "Fixing with Claude
  · 2 of 4" with the working spinner while [Fix It](/documentation/features/fix_it.md) runs, or
  "Subtitles ready · fixed by Claude" once a fixed job passes with nothing left to check. A job
  finished in an earlier window shows the same line. A click selects a row, a waiting row drags to
  another place in line, and a right click opens the commands of its state: Run Next, Move Up, Move
  Down, Cancel, Stop Updating Subtitles, Check Lines, Open in Player, Show in Folder, Copy Subtitle
  Path, Run Again with Current Settings, Try Again, Remove from List. An empty list says "No videos
  yet", and the detail pane shows a card to drop videos on or add them with its buttons.
- **Header:** the selected job's name in the 22 px title, and under it its length and how long it
  ran ("25:59 video · running for 10 min 00 s") or its place in line. On the right: Cancel (then
  "Stopping…") for a running job, or Overview | Check Lines | Check Text for a finished one;
  Check Lines keeps its count of lines to check, or a green check once none is left.
- **Job cards:** a running job's card says what its [stage](/documentation/glossary.md#stage)
  does ("Settling the words"), the [step](/documentation/glossary.md#step) at work ("Now: Language
  model settles the words · step 9 of 28"), a bar of the share done, the time left ("about 4 min
  left", "Working out the time left…" until the length is known; steps the job's settings leave
  idle add none) and the time so far. Under it nine groups: Read the video, Separate the voices,
  Hear the speech, Settle the words, Time the words, Lay out the subtitles, Translate on-screen
  text, Write the subtitles and Write the localized video. Each shows its time and state; the
  running group opens to its steps. "Show all 28 steps" opens every group.
  A waiting job's card gives its place, when it starts, its path, Run Next and Remove from List. A
  failed job's card names the stage and step in plain words ("Failed at Hear the speech", "Listen
  with Whisper stopped with an error."), the raw message, the finished steps kept and when Try
  Again starts it, with Try Again and Show in Folder. A cancelled job's card gives the finished
  steps kept, with Try Again and Remove from List.
- **Toasts:** short messages at the bottom centre for what the window shows nowhere else, some
  with a button such as Undo; errors are red.
- **Overview** (a finished job): the file card says "Subtitles saved next to the video" with the
  pill "Passes the quality check" or "Needs attention"; right under it, once Fix It has run, the
  green [Result card](/documentation/features/fix_it.md#the-result-card) ("Fixed by Claude Opus",
  what it changed, cleared and left, See Changes). The card lists each whole-video problem with its
  fix, shows a blue note while a correction run updates the file or
  [Fix It](/documentation/features/fix_it.md) runs ("Fixing with Claude Opus · updating the
  subtitles (4 of 4)…", or "Waiting for a free Claude call. 3 of 12 calls done." while every call of
  its run waits under the cap on Claude calls at once), the Fix It row with its button while a
  flagged finding is one Claude has not answered, the path, and Open in Player (the desktop's
  default player, through the desktop portal), Show in Folder and Copy Path, each confirmed in a
  toast (a red one when the desktop could not, or has not answered within 20 seconds). Each request
  to the desktop portal has a D-Bus connection of its own, so one the desktop never answers holds up
  no other. The lines card says "38 lines worth a listen" with a green bar of those checked, Check
  Lines, and one row per finding group with its explanation and count. A line Fix It changed or
  answered counts as checked; with none left, the card says "All 38 lines checked" and "Claude
  checked 36 · you checked 2". Then Details (subtitles, easy to read, unsure lines, words no engine
  heard, timing offset, speech and voice with no subtitle, words timed by the aligner, and
  "Corrections you made", the owner's alone, not Claude's) and Step times (each stage and step with
  its time, peak RAM and peak VRAM, and Open Full Report), both folded away at first.
  On-screen text adds detected, translated, nearby and unresolved counts ("· 8 replaced in the
  video" once the replacement steps ran), visual processing time, combined ASS output and a Check
  Text action; warnings remain visible after the job finishes. While a job's localized video is on
  disk, a card under the file card says "Localized video saved next to the original" ("The
  Japanese writing is replaced in English where it could be; the rest is in its own subtitle file.
  The original video is unchanged."), with the video's and its subtitle file's paths (whole on
  hover) and Open in Player, Show in Folder and Copy Path for the video.
- **Check Lines** (a finished job):

  ```text
  ┌─ line list, 330 px ────────────┬─ open line ──────────────────────────────────────────────┐
  │ [To Check 38|Checked 2|All 412]│ 16:33.4 · U0412 · 2.4 s          (Updating subtitles…)   │
  │ [Search text or time        ]  │ ! Heard word replaced                                    │
  │ Showing (Word replaced x)      │ Both engines heard “Frankie”; the subtitles don't use it │
  │ 16:33.4 Hey, Franky!         o │ [still frame, at most 480 px wide]                       │
  │   [Word replaced]              │ |//pad//|====== the line ======|//pad//|   red playhead  │
  │ 16:40.1 Where's the ...      o │ [Play] [Voices Only]                                     │
  │   [Unsure] [Too fast]          │ IN THE SUBTITLES NOW   Hey, Franky!                      │
  │ 17:02.9 I'll take him on.    v │ WHAT WAS HEARD   Language model's pick  [In use]         │
  │   [Corrected]                  │                  Parakeet   Hey, Frankie!  [Use]         │
  │                                │                  Whisper    Hey Franky     [Use]         │
  │                                │ YOUR TEXT  [Hey, Franky!                              ]  │
  │                                │ [ ] New speaker  [ ] Narrator  [ ] Song lyric  [ ] Drop  │
  │                                ├──────────────────────────────────────────────────────────┤
  │                                │ [Previous] [Next]                        [Looks Right]   │
  └────────────────────────────────┴──────────────────────────────────────────────────────────┘
  ```

  The list shows To Check, Checked or All with their counts (To Check matches the header's
  count), a search field (words, a line's id, or a time such as `16:33`), the group the list is
  narrowed to as an orange pill with ✕, and a row per line: its time, its text cut after two
  lines, a dot (orange to check, blue while edited), a wand (changed by Fix It) or a green check,
  and chips (its groups, "Claude" first for a Fix It change, or "Edited, not saved", "Looks
  right", "Corrected"). The open line shows its time and id with the status chip of its
  correction run; a blue box with what the app had and why for a Fix It change, an orange box per
  finding group saying why, or a green one once kept or corrected; the clip with its picture (the frame at the line's start, then the
  clip's frames), a timeline with hatched 0.75 s pads, the line's span and a red playhead, and
  Play, Voices Only or Stop; "In the subtitles now"; what was heard (the language model's pick,
  Parakeet, Whisper, and each engine's second listen to the voices alone), each with Use or "In
  use"; the text box ("Type || where a second speaker starts."); and four switches: New speaker
  (never merged into the line before; they share a subtitle only with dashes), Narrator (in
  italics, never in a two-speaker subtitle), Song lyric (left out of the dialogue; its song can
  get a [sound cue](/documentation/glossary.md#sound-cue)) and Drop the line (left out). The
  footer has Previous and Next (off at the list's ends), then Discard Edit and Save Correction for
  an edited line, Keep Change and Undo Change for a Fix It change, Take Back for a checked one,
  or Looks Right for an unedited one. Nothing is saved while a full run of the video runs, and
  the owner's corrections change only lines the owner saved; Fix It changes others. With every
  line checked the pane says "All 38 lines checked" and "The subtitles are up to date.", with Show
  Checked Lines.
- **Check Text:** thumbnails and timestamps beside Japanese, English, confidence, provenance and
  review status; flagged-only filtering, an English/timing/placement/size/treatment editor, and
  Original/English comparison rendered from the actual ASS by FFmpeg. Play, scrub and frame-step;
  Save & regenerate ASS, Undo or Retry selected text. Saves survive navigation and update their
  owning video through the existing correction queue without replacing another open session.
  For a job with the [localized video](/documentation/glossary.md#localized-video), a Subtitles |
  Localized video control over the right picture switches it between the source with its ASS
  and the localized video with its own subtitle file (the default, kept across reloads); until the
  video is written it shows the occurrence's replaced keyframe plate, captioned "Localized video
  not written yet". Show erase mask, beside the original picture's title, tints the keyframe's
  [stroke mask](/documentation/glossary.md#stroke-mask) over the original; a line under the time
  says "Replaced in the video" (green) or `Not replaced in the video: <reason>` (orange), and the
  Replace treatment reads "Replace in the video".
- **Settings window:** a second native window, centred over the main one when it opens, in six
  tabs. General: the models folder and the work folder with their sizes, the subtitle format, the
  glossary with its count of names. Engines: vocal separation, the second speech engine, the
  `claude` model a run asks and the one Fix It asks (Sonnet, Opus, Fable or Haiku; Sonnet and Opus
  by default), processes at once, Fix It's Claude calls at once (1–100, 32 by default: how many
  `claude` calls Fix It makes at once across every video it fixes; the rest wait their turn, videos
  started first going first) and the switch Fix It after each job (off by default: Fix It starts on
  each video when its job finishes, if it has lines to fix), the shot cut score. On-screen Text:
  enable translation, Replace text in the video (on by default, off to clicks while translation is
  off; `<name>.localized.mkv` and `<name>.localized.ass` beside the original), local-first policy,
  Claude fallback, reference-folder path and model status with download controls. Enabled visual
  translation produces ASS. Models: each model
  and runtime library with its size and state, Download Missing or Stop. This Computer: the GPU with
  its driver and free VRAM, the CUDA libraries, FFmpeg and its clip sound, ffprobe, `claude`, the
  Whisper worker; a missing CUDA runtime links to Models. Automation: the watch folders, with Add
  Folder…, Remove and "Not found" for a missing one, and whether Dolphin's right-click entry
  "Generate subtitles" is installed, with its path
  ([automation](/documentation/features/automation.md)). A change is saved to `settings.toml` as it
  is made (a list or the format on a choice, a stepper on each press, a typed number on Enter, when
  it loses the focus or when the window closes) and applies to videos that have not started, as the
  footer says. A number out of its range, a glossary that cannot be read, or a models folder moved
  while a download runs is not saved and says why in red under its field; a `settings.toml` that
  could not be read is kept as `settings.toml.broken` before the first change is saved. Folders show
  the home as `~`.

### The log window

The log button or Ctrl+L opens a 980 by 560 native log window, including OCR/model calls and visual failures,
near the main window's lower right corner; asked for again while open, it comes to the front. It
has two views: **Activity**, every line logged since the window started, and **Model Calls**,
every language-model call with what was sent and what came back.

```text
┌────────────────────────────────────────────────────────────────────────────────────────────┐
│ [ Activity | Model Calls 3 ]                            [Copy] [Clear] [Open Log File]     │
│ [Errors 1 | Warnings 1 | Info | Debug]  [Everyone | App | Jobs | AI | Programs]  [Filter]  │
├────────────────────────────────────────────────────────────────────────────────────────────┤
│ 04:10.101  (App)      action Queue(Start)                                                  │
│ Dressrosa 12 · Hear the speech — Listen with Parakeet                                      │
│ 04:12.300  (Job)      Step started                                                         │
│ 04:12.310  (Program)  tbd-subtitles[4242] started: /app/tbd-subtitles worker asr_parak…    │
│ 05:40.870  (Job)      Step finished in 88.5 s, 1830 MiB RAM, 2410 MiB VRAM                 │
│ Dressrosa 12 · Settle the words — Language model settles the words                         │
│ 06:02.114  (AI)       claude sonnet · words, batch 1 of 40: 212 lines answered in 41.3…    │
│ 06:02.500  (Program)  ! ffprobe[4302] exited 1 after 0.05 s                                │
├────────────────────────────────────────────────────────────────────────────────────────────┤
│ INFO  AI  inference::llm::call_log  06:02.114     [Show Model Call] [Copy] [x]             │
│ Dressrosa 12 · Settle the words — Language model settles the words                         │
│ claude sonnet · words, batch 1 of 40: 212 lines answered in 41.3 s, 18234 tokens in, …     │
├────────────────────────────────────────────────────────────────────────────────────────────┤
│ 7 lines                  Times since the window opened · keeps the newest 20000 lines      │
└────────────────────────────────────────────────────────────────────────────────────────────┘
```

- **Activity:** one row per line, never wider than the window: the time since the window
  opened, a chip for who wrote it (**AI**, the language model and Fix It, in blue; **Job**, the
  pipeline's steps, in green; **Program**, FFmpeg, ffprobe, `claude` or a worker, in grey; **App**,
  the window itself, outlined), a mark for a warning (orange) or an error (red), and the message,
  cut with `…` where the window ends. Debug lines are grey. A header row names the video and the
  step in words each time the work moves on, so a step's own lines, its worker's and its programs'
  sit under it; the owner's own actions never break a group. A worker's own log lines show with
  the worker's level and source, not as program output.
- **What it shows:** the owner's actions; each job's start, steps (started, kept, finished with
  time, RAM and VRAM, failed) and end, a step's progress every tenth; Fix It's passes and end,
  and an answer reused from an earlier run; where each step runs, the CUDA runtime and the GPU
  lock; one line per model call (model, why it was made, lines sent, seconds, tokens, cost); and
  every program the app starts: its command line, each line it writes to stderr as it writes it,
  and how it ended.
- **A line whole:** a click opens the line in a panel below the list: its level, writer, source
  and time, its video and step, and the whole message wrapped, with Copy; a model call's line
  offers **Show Model Call**. ✕ or Esc closes it.
- **Model Calls:** the calls on the left, each with why it was made ("words, batch 3 of 40",
  "words heard again, round 1", "sound cues, window 2 of 9", "words fixes, call 3 of 8"), its
  time, video and step, model and seconds; a failed one in red. The open call shows its model,
  id, seconds, tokens and cost, and why it failed; then **Answer** (open at first), **Message**,
  **System prompt** and **Schema**, each foldable with its size and Copy, the text whole. Copy in
  the bar copies the whole call. Calls made in a worker reach the window through the worker's
  stdout; Fix It's are made in the window itself.
- **Controls:** the levels show that level and the more severe (Debug shows everything; Errors
  and Warnings count theirs); the writers show one kind or everyone; the search keeps the lines
  whose text, source, video or step holds it, and the calls whose model, purpose, video or step
  holds it, ignoring case. Copy puts the shown lines, or the open call, on the clipboard; Clear
  empties the view shown (the log file keeps every line); Open Log File opens the log file in the
  desktop's text editor. The footer counts what shows of what is kept.
- The window keeps the newest 20,000 lines and 500 calls; what is logged while it is closed shows
  when it opens. The log file holds the same lines, emptied at each start, and never a call's
  prompt or answer: calls are kept in memory only. `RUST_LOG` narrows the lines when set, never
  the calls.

### Main flow

```text
 Add / Drop / CLI -> Up Next -> Start Queue -> Now: audio and visual steps -> Done
                           models required        |                          |
                            Cancel / failure -> Try Again      Overview / Check Lines / Check Text
                                                                corrections -> same video's update
```

- One job runs at a time and its GPU stages run one after another. Beside it, up to four
  correction runs of different videos run at once, each on a review lane of its own; the rest
  wait for a lane. A job takes the settings
  saved now until it first starts, and its own from then on; the queue is kept across windows.
- Try Again puts a failed or cancelled job first in line and resumes after the steps it kept.
  Run Again with Current Settings runs only the steps the settings saved now change, and says so
  in a toast when nothing changed. Either starts at once when nothing runs, without turning the
  queue on, and neither puts back a video that is already in the list.
- A full run of a video waits while a correction run of the same video runs, and the reverse; a
  second correction run of a video waits while its first runs, even with a lane free.
- The queue runs on while the window is minimized. A later launch of the app raises this window
  and queues its videos; a watch folder or Dolphin's entry queues and starts; a job that ends
  while the window is away gives a notification, naming the localized video when it saved one
  ([automation](/documentation/features/automation.md)).

### Overview to Check Lines

```text
┌─ file card ────────────────────────────────────────────────────────────────────────────────┐
│ Subtitles saved next to the video                                        (Needs attention) │
│ ┌ (seal) Fixed by Claude Opus ──────────────────────────── the Result card, after Fix It ┐ │
│ │ 17 lines changed · 21 were already right · cleared, left, examples       [See Changes] │ │
│ └────────────────────────────────────────────────────────────────────────────────────────┘ │
│ each whole-video problem, with its fix                                                     │
│ blue note: "Fixing with Claude Opus · updating the subtitles (4 of 4)…"                    │
│ Fix It with Claude Opus · while Claude has not answered a flagged finding         [Fix It] │
│ the path                                     [Open in Player] [Show in Folder] [Copy Path] │
└────────────────────────────────────────────────────────────────────────────────────────────┘
┌─ lines card ───────────────────────────────────────────────────────────────────────────────┐
│ 38 lines worth a listen   ██████████░░░░░░░░░░                               [Check Lines] │
│ with none left: "All 38 lines checked" · "Claude checked 36 · you checked 2"               │
│ one row per finding group, with its explanation and count                                  │
└────────────────────────────────────────────────────────────────────────────────────────────┘
  Details and Step times, folded away at first
  [See Changes] ──▶ Check Lines on the Changed by Claude group
```

An edit not saved, and the status chip of a saved line, stay while the window is open, also when
Check Lines closes; a finding stays worth a listen until the correction run that settles it ends.

### What each finding group means

The quality check flags lines for a listen in six groups, and a seventh, first, holds the lines
[Fix It](/documentation/features/fix_it.md) changed that the owner has not kept or undone (chip
"Claude"). Only layout findings, and too many lines too fast to read, can fail a job; each line's
box in Check Lines says which word or number put it there.

- **Unsure what was said** (chip "Unsure"): the line is flagged
  [UNSURE](/documentation/glossary.md#unsure) by
  [adjudication](/documentation/glossary.md#adjudication).
  - Already in the file: the language model's best guess.
  - Why flagged: the two speech engines disagreed, and a second listen to the voices alone did
    not settle it.
  - What you can do: play the clip, Voices Only when music is loud; Use the reading that matches
    or type the line, then Save Correction; Looks Right when the guess is right.
- **Heard word replaced** (chip "Word replaced"; the check `removed_locked`):
  - Already in the file: the line without a word both engines heard, nearly always a name spelled
    to match the name glossary (Frankie → Franky).
  - Why flagged: "Both engines heard “Frankie”; the subtitles don't use it."
  - What you can do: Looks Right when the glossary spelling is right; otherwise Use the engine's
    reading or type the word.
- **Word no engine heard** (the check `novel`):
  - Already in the file: a word neither engine heard, usually a name the language model took from
    the glossary.
  - Why flagged: every subtitle word is to come from what a speech engine heard, so a word from
    elsewhere is shown to the owner. Its box says "Neither engine heard" and names the word.
  - What you can do: Looks Right when the word was said; otherwise Use a reading or type the line.
- **Too fast to read** (chip "Too fast"):
  - Already in the file: the line as heard, in a [cue](/documentation/glossary.md#cue) over 20
    [characters per second](/documentation/glossary.md#cps) ("23.4 characters per second; the
    limit is 20.").
  - Why flagged: the reading-speed rule; the job fails only when fewer than 95 % of its subtitles
    are within 20.
  - What you can do: optional. Type a shorter wording the speaker's words allow and Save
    Correction, or leave it.
- **Loosely timed:**
  - Already in the file: a line whose words are timed from the
    [backbone engine](/documentation/glossary.md#backbone-engine)'s own times or spread between
    timed neighbours ([timing source](/documentation/glossary.md#timing-source)).
  - Why flagged: fewer than half its words were timed by
    [forced alignment](/documentation/glossary.md#forced-alignment).
  - What you can do: Looks Right, which re-times the line alone with the aligner.
- **Layout:**
  - Already in the file: the subtitle as built, which breaks a layout rule (two cues at once,
    fewer than 2 frames between cues, shorter than 5/6 s or longer than 7 s, a line over 42
    characters, more than two lines, no text, or ending after the video).
  - Why flagged: the [subtitle style rules](/documentation/architecture/subtitle_style_rules.md);
    an overlap, a short cue, a long line, a third line, an empty cue or one past the end fails the
    job.
  - What you can do: open the line and edit it; saving rebuilds its subtitles.

A finding about a cue opens the line whose words overlap that cue the longest. A finding on a
sound cue with no words has no line to open: it is no line to check, but it still counts in the
file card's layout problem and is listed in `report.md`.

### Whole-video problems

A finished job passes the quality check when it has none of these; each is a row of the file card
with its fix, and the sidebar row counts them ("Needs attention · 1 problem").

| Problem | What to do | Button |
|---|---|---|
| "N subtitles break a layout rule" | Open them in Check Lines; editing a line rebuilds its subtitles. | none |
| "Speech with no subtitle" | Show the lines around the first stretch of heard speech with no cue. | Show Nearby Lines |
| "The aligner's timing is off by 30 ms or more" | Every subtitle may sit slightly early or late; there is no per-line fix. | none |
| "N language-model calls failed" | Some lines may be missing; run adjudication and the steps after it again. | Try Again |
| "Only 91.2 % of subtitles are easy to read" | The target is 95 % within 20 characters per second; shorten some Too fast lines. | Show Lines |

Fix It asks its model about every one of these but the aligner's offset and failed calls, and
about the lines worth a listen ([Fix It](/documentation/features/fix_it.md)).

### Keyboard shortcuts

| Keys | Where | What they do |
|---|---|---|
| Ctrl+O | anywhere | Add Videos… |
| Ctrl+Shift+O | anywhere | Add Folder… |
| Ctrl+, | anywhere | open Settings, or bring them to the front |
| Ctrl+L | anywhere | open the log window, or bring it to the front |
| ↑ and ↓ | the sidebar | select the row above or below |
| Delete | the sidebar | remove the selected row (Undo in the toast puts it back) |
| ↑ and ↓ | Check Lines | open the line above or below |
| Space | Check Lines | play the clip, or stop it; a focused button takes Space for itself |
| Ctrl+S | Check Lines | Save Correction on an edited line |
| Ctrl+Enter | Check Lines | Looks Right on an unedited line; Keep Change on a Fix It change |
| Esc | Check Lines | leave the text box; then stop the clip |

Space, Delete and the arrows belong to the text box while typing and do nothing while a menu is
open; in Check Lines the arrows move through the lines, not the sidebar, and Delete removes
nothing.

## Data

- Settings: `~/.config/tbd-subtitles/settings.toml`, read when the window opens and written on
  each change; a broken one is kept as `settings.toml.broken`.
- The queue: `~/.local/share/tbd-subtitles/queue.json`, written on each change and read when the
  window opens.
- Models and the CUDA runtime libraries: under `~/.local/share/tbd-subtitles/`, the models in
  the settings' models folder when it names another.
- Jobs: each job's [work directory](/documentation/glossary.md#work-directory)
  ([system overview](/documentation/architecture/system_overview.md#job-work-directory)). The
  window reads `job.json` and the step records for progress and the time left, `qc.json`,
  `output.json`, `report.md` and `fix.json` for the Overview, and `sheet.json`, `adjudicated.json`, the
  re-decodes, `probe.json` and `audio/vocals_16k.f32` for Check Lines. Check Text reads the visual
  documents/crops and exported ASS (and `visual/text_compose.json`, `visual/localized_video.json`,
  the masks and previews for the localized video), and writes locked `visual/corrections.json`. Line review writes
  `review.json`, the corrections the review step reads, under its lock `review.json.lock`, which
  Fix It takes too; Fix It also writes `fix.json` and, while it runs, `fix/calls/`. Each finished
  row's verdict and lines to check are read from `qc.json` and `review.json` when the window opens
  and after each run and correction of its video.
- The log: `~/.local/state/tbd-subtitles/tbd-subtitles.log` (under `XDG_STATE_HOME` when set),
  written as the window runs, since a launcher such as Gear Lever drops stderr, and emptied at each
  start; the [log window](#the-log-window) shows the same lines from memory.
- Kept only while the window is open: unsaved line edits, correction-run status chips, the row
  removed last (for Undo), toasts, and which sidebar sections are folded.

## Design

- **Look:** the mockup's light and dark palettes (a macOS blue accent, greys, green, orange and
  red whose text reads at a contrast of at least 4.5), Adwaita Sans from the system in regular,
  semibold and bold and Adwaita Mono for monospace text (egui's fonts when they are missing),
  Phosphor icons, title 22, headline 15, body 13 and caption 11, controls 28 px high with radius
  6, cards and windows with radius 10, 1 px borders and one soft shadow, and selections in a light
  accent tint with accent text. The window follows the desktop's light or dark colour scheme as
  KDE sets it, through the desktop portal, and switches when it changes.
- **X11:** the window runs under XWayland because only there can files be dropped onto it and
  Settings be centred over it; at the owner's scale factor of 1 XWayland draws the same pixels,
  as smoothly. The renderer is glow (OpenGL), because wgpu fails to create a surface on the
  owner's Wayland desktop.
- **Built differently from the mockup:**
  - The clip's playhead follows the time since the sound started, so it may lead the audio by
    tens of milliseconds.
  - Looks Right re-times the line on its own; its hover text says so.
  - A layout finding opens the line whose words overlap its subtitle the longest; a finding on a
    sound cue with no words cannot be opened.
  - The models banner says "models and runtime libraries" when both are missing.
  - Unsaved line edits are kept while the window is open, not across restarts.
  - The accent colour is macOS blue as mocked; KDE's accent colour is not followed.
- **Code:** one folder per feature with `models/`, `services/` and `ui/`, following the
  TBD-Reforger desktop-app pattern: the UI draws from a borrowed view and returns events that the
  application applies after the frame, so nothing changes state while a frame is drawn. The
  architecture tests (`apps/tbd_subtitles/src/tests/architecture_rules.rs`) hold the pattern.
- **Tests:** headless tests render the frame and assert what it paints; an ignored test renders
  the real window offscreen at 1280 by 800, light and dark, from a copy of real work folders
  (`apps/tbd_subtitles/src/application/tests/window_snapshots.rs`), to compare with the mockup.

## Open work

- M2 and [M3](/documentation/roadmap.md#m3--automation) are complete. [M4](/documentation/roadmap.md#m4--japanese-on-screen-text)
  still requires annotated pilots, resource benchmarks, full GUI/playback checks and owner acceptance;
  [M5](/documentation/roadmap.md#m5--in-place-on-screen-text) its playback checks and the owner's acceptance.

## Decisions

- eframe over iced or Slint, and clips through FFmpeg ([clips play through FFmpeg](/documentation/decisions/desktop_gui.md#2026-09-26--clips-play-through-ffmpeg-not-libmpv)).
- The desktop portal for choosers and for opening a video ([the desktop portal](/documentation/decisions/desktop_gui.md#2026-09-26--the-desktop-portal-chooses-files-and-opens-videos)).
- A review step times the owner's corrections ([review step](/documentation/decisions/desktop_gui.md#2026-09-26--the-owners-corrections-are-timed-by-a-review-step)).
- Models live in the settings' models folder and download from Settings ([models folder](/documentation/decisions/desktop_gui.md#2026-09-26--models-live-in-the-settings-models-folder-and-download-from-the-settings-page)).
- A job passes the quality check on five rules ([five rules](/documentation/decisions/desktop_gui.md#2026-09-26--a-job-passes-the-quality-check-on-five-rules)).
- One GPU worker at a time on the machine ([one GPU worker](/documentation/decisions/desktop_gui.md#2026-09-26--one-gpu-worker-at-a-time-on-the-machine)).
- The owner chooses SRT, WebVTT or ASS; enabled on-screen translation selects combined ASS ([subtitle format](/documentation/decisions/desktop_gui.md#2026-09-26--the-owner-chooses-srt-webvtt-or-ass)).
- The desktop's colour scheme, Adwaita Sans, Phosphor icons and X11 ([the window's look](/documentation/decisions/desktop_gui.md#2026-09-28--the-window-follows-the-desktops-colour-scheme-in-adwaita-sans-under-x11)).
- A job keeps its own settings once it has started ([a job keeps its settings](/documentation/decisions/desktop_gui.md#2026-09-28--a-job-keeps-its-own-settings-once-it-has-started)).
- Settings in their own window ([Settings window](/documentation/decisions/desktop_gui.md#2026-09-28--settings-open-in-a-window-of-their-own)).
- Correction runs on their video's row ([one row per video](/documentation/decisions/desktop_gui.md#2026-09-28--correction-runs-show-in-their-videos-row)).
- Try Again resumes after the kept steps and starts at once ([Try Again](/documentation/decisions/desktop_gui.md#2026-09-28--try-again-resumes-after-the-kept-steps-and-starts-at-once-when-nothing-runs)), and reruns adjudication for a failed language-model call ([a failed call is retried](/documentation/decisions/desktop_gui.md#2026-09-28--a-failed-language-model-call-is-retried-by-running-adjudication-again)).
- Settings apply as they change, with no Save ([settings apply as they change](/documentation/decisions/desktop_gui.md#2026-09-28--settings-apply-as-they-change)).
- Fix It fixes the flagged lines in three passes, each change kept by the owner or undone ([Fix It](/documentation/decisions/desktop_gui.md#2026-09-28--fix-it-a-stronger-model-fixes-the-flagged-lines-in-three-passes)), and shows its result in a Result card when the subtitles are updated ([Fix It finishes visibly](/documentation/decisions/desktop_gui.md#2026-09-28--fix-it-finishes-visibly)); it runs on many videos at once under one cap on Claude calls, with Fix All and Fix It after each job ([Fix It on many videos](/documentation/decisions/batch.md#2026-09-28--fix-it-runs-on-many-videos-at-once-under-one-cap-on-claude-calls)).
- A log window shows everything the app does, from one `tracing` buffer ([log window](/documentation/decisions/desktop_gui.md#2026-09-28--a-log-window-shows-everything-the-app-does)).
- Model calls show whole in the log window, never in the log file ([model calls](/documentation/decisions/desktop_gui.md#2026-09-28--model-calls-show-in-the-log-window-never-in-the-log-file)).
- One window per session, work while minimized, and job-end notifications ([automation decisions](/documentation/decisions/automation.md)).
- On-screen writing is replaced in a localized video beside the source, on by default ([localized video](/documentation/decisions/stack_and_pipeline.md#2026-09-30--writing-is-replaced-in-a-localized-video-re-encoded-beside-the-source)).
