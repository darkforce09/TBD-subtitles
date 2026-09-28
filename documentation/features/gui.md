**Status:** live

# Desktop GUI

The window the owner uses to queue videos, watch progress, read job reports and fix the lines the
pipeline was unsure about; milestone M2. What exists now: the window opens (`tbd-subtitles` or
`tbd-subtitles gui [VIDEO]...`) with a toolbar across the top, a sidebar of videos on the left
and the selected job on the right. The sidebar lists the videos given on the command line, dropped
onto the window or added with Add Videos… and Add Folder… through the desktop's chooser (files,
or a folder for its videos without subtitles), one row per video in the sections Now, Up Next and
Done; a row leaves the list by its ✕, its menu or Delete, and Undo puts it back. Settings open in
a window of their own from the gear (Ctrl+,), in four tabs that save each change as it is made:
they edit `settings.toml`, list and download the models, and check the machine; a banner under
the toolbar says when models are missing and downloads them. Start Queue runs the waiting jobs
one at a time; the selected job's header names it with its length and time, and its cards show
the stage at work, the step ("step 9 of 18"), the time left and the six stages; Pause After This
Video stops the queue once
the running video ends. A running job can be cancelled and an ended one tried again, a failed job
names the stage and step it failed at with the raw message, a failed or cancelled one says how
many finished steps it kept and when Try Again would start it, and the queue is kept across
windows. A finished job's Overview says the subtitles are saved next to the video and whether
they pass the quality check, each problem in plain words with its fix (Try Again beside a failed
language-model call), how many lines are worth a listen in their groups and how many are checked,
the check's numbers and each step's time and memory, with buttons that open the video, show the
file in its folder, copy its path and open `report.md` through the desktop; its sidebar row says
"Subtitles ready · 38 to check" with an orange count, or "Needs attention · 1 problem", even for
a job finished in an earlier window. Its Check Lines tab opens the line review: a list of the
lines to check, those checked or every line, narrowed to a group or a search, beside the open
line with why it is worth a listen, its clip (the video's sound or the voices alone, its picture
and a playhead), what every engine heard, the text and its flags. The owner uses a reading or
types the line, and Save Correction, or Looks Right for a line that is right as it is, writes
`review.json`, moves on to the next line and starts a review run at once, which re-times the
saved lines and rewrites the subtitle file; Take Back undoes one. A status chip over the line
goes Saved → Updating subtitles… → Subtitles updated, and the video's row shows the review run
("Updating subtitles · 1 correction") until it ends.

## Where it lives

- Code: `apps/tbd_subtitles/`, subcommand `gui`, built with eframe (egui) on the glow renderer:
  the shell in `apps/tbd_subtitles/src/application/`, and one feature folder each for the queue
  (`apps/tbd_subtitles/src/job_queue/`), the report (`apps/tbd_subtitles/src/job_report/`),
  line review (`apps/tbd_subtitles/src/line_review/`) and settings
  (`apps/tbd_subtitles/src/settings/`).
- Entry: `tbd-subtitles gui` from a terminal on the host; a desktop entry "TBD Subtitles" is
  planned with the automation feature.
- Related: [automation](/documentation/features/automation.md) feeds the same job queue.

## Behaviour

1. **Queue.** Add videos by file picker (Ctrl+O), drag and drop (an overlay shows while files
   hover), or folder (Ctrl+Shift+O; every video without a subtitle file). The toolbar's one
   button is Start Queue, Pause After This Video or Resume Queue, and says why Start Queue is off
   ("Download the models first", "Nothing is waiting"); it counts full runs only. The sidebar has
   one row per video, a review run folded into its video's row, with a status mark and line
   ("Waiting · 2nd in line", "Failed at Hear the speech", "Subtitles ready · 38 to check",
   "Needs attention · 1 problem"; a finished row that passes ends in the orange count of its lines
   to check), the newest ended first in Done; a click
   selects a row, ↑ and ↓ move through them, a waiting row drags to another place in line, and a
   right click opens the commands of its state: Run Next, Move Up, Move Down, Cancel, Stop
   Updating Subtitles, Check Lines, Open in Player, Show in Folder, Copy Subtitle Path, Run Again
   with Current Settings, Try Again, Remove from List. Try Again puts a failed or cancelled job
   first in line and resumes after the finished steps it kept; Run Again with Current Settings runs
   only the steps the settings saved now change, and says so when nothing changed; either starts at
   once when nothing runs, without turning the queue on, and neither puts back a video that is
   already in the list.
   One job runs at a time; GPU stages run one after another. A job takes the settings saved now
   until it first starts, and its own from then on. A full run of a video waits while a review
   run of the same video runs, and the reverse. Toasts at the bottom centre report what the window
   shows nowhere else, with a button such as Undo; errors are red.
2. **Progress.** The selected job's header gives its name, its length and how long it ran
   ("25:59 video · running for 10 min 00 s", "25:59 video · finished in 4 min 37 s") or its place
   in line, and on the right Cancel (then "Stopping…") for a running job or Overview | Check Lines
   for a finished one, Check Lines with its count of lines to check (a green check once none is
   left). A running job's card says what its stage does ("Settling the words"), the
   step at work ("Now: Language model settles the words · step 9 of 18"), a bar of the share
   done, the time left ("about 4 min left", "Working out the time left…" until the length is
   known) and the time so far; under it the six stages, each done with its time, running with its
   time so far, failed, or still to come, the running one open to its steps and "Show all 18
   steps" opening every stage. A waiting job's card gives its place, when it starts, its path, Run
   Next and Remove from List. A failed job's card gives its stage and step in plain words ("Failed
   at Hear the speech", "Listen with Whisper stopped with an error."), the raw message, the
   finished steps kept and when Try Again starts it, with Try Again and Show in Folder, over its
   stages; a cancelled job's card gives the finished steps kept and when Try Again starts it, with
   Try Again and Remove from List.
3. **Report.** When a job ends, under its Overview tab. The file card: "Subtitles saved next to
   the video" with the pill "Passes the quality check" or "Needs attention"; each broken pass rule
   in plain words with its fix ("1 language-model call failed" with Try Again, which runs the
   language-model calls and every step after them again; "Speech with no subtitle" with Show
   Nearby Lines, which opens every line at the one nearest the first stretch; "Only 91.2 % of subtitles are easy to read" with Show Lines; layout; the
   aligner's offset); a blue note while a correction run updates the file; the path; Open in
   Player (the desktop's default player, VLC, through the desktop portal), Show in Folder and Copy
   Path, each saying in a toast what it did, and in a red one when the desktop could not. The
   lines card: "38 lines worth a listen", a green bar of those checked, Check Lines, and a row per
   group (Unsure what was said, Heard word replaced, Word no engine heard, Too fast to read,
   Loosely timed, Layout) with its explanation and count, which opens Check Lines narrowed to that
   group. Then Details (subtitles, easy to read, unsure lines, words no engine heard,
   timing offset, speech and voice with no subtitle, words timed by the aligner, corrections made)
   and Step times (each stage and step with its time, peak RAM and peak VRAM, and Open Full
   Report), both folded away at first.
4. **Review.** Under a finished job's Check Lines tab. On the left, 330 px wide: To Check,
   Checked and All with their counts (To Check matches the header's count; a line stays worth a
   listen after its correction run settles its findings, and while a taken-back line's run runs),
   the search field
   (words, a line's id, or a time such as `16:33`), the group the list is narrowed to as an orange
   pill with ✕, and a row per line with its time (`16:33.4`), its text cut after two lines, a dot
   (orange to check, blue while edited) or a green check, and chips: its groups ("Unsure", "Word
   replaced", "Too fast"), or "Edited, not saved", "Looks right" or "Corrected". On the right the
   open line: its time and id with the status chip of its correction run; an orange box per group
   saying why ("Both engines heard “Frankie”; the subtitles don't use it."), or a green one once
   kept or corrected; the clip, its picture at most 480 px wide (the frame at the line's start,
   decoded when the line opens, then the clip's frames), a timeline with hatched 0.75 s pads, the line's span and a
   red playhead that follows the time since the sound began, and Play, Voices Only or Stop; "In
   the subtitles now"; what was heard (the language model's pick, Parakeet, Whisper, and each
   engine's second listen to the voices alone), each with Use or "In use"; the text box ("Type ||
   where a second speaker starts."); and four switches that say what the subtitles do: New
   speaker (never merged into the line before; they share a subtitle only with dashes), Narrator
   (in italics, never in a two-speaker subtitle), Song lyric (left out of the dialogue; its song
   can get a sound cue) and Drop the line (left out). The footer has Previous and Next (off at the
   list's ends), then Discard Edit and Save Correction for an edited line, Take Back for a checked
   one, or Looks Right, which saves the line unchanged so it is timed again and its warnings
   clear. Saving moves on to the next line (or stays on the last); an edit not saved, and the
   status chip of a saved line, stay while the window is open, also when Check Lines closes. Keys: Ctrl+S saves an edited line, Ctrl+Enter keeps an
   unedited one, ↑ and ↓ move through the lines, Space plays or stops (or presses the button that
   has the keyboard's focus), Esc leaves the text box and
   then stops the clip; Space and the arrows are the text box's while typing. Nothing is saved
   while a full run of the video runs. Corrections never touch lines the owner did not save.
5. **Settings.** In a window of their own, centred over the main one when they open, in four
   tabs: General (the models folder and the work folder with their sizes, the subtitle format,
   the glossary with its count of names), Engines (vocal separation, the second speech engine,
   the `claude` model, processes at once, the shot cut score), Models (each model and runtime
   library with its size and state, Download Missing or Stop) and This Computer (the GPU with its
   driver and free VRAM, the CUDA libraries, FFmpeg and its clip sound, ffprobe, `claude`, the
   Whisper worker; a missing CUDA runtime links to Models). A change is saved to `settings.toml`
   as it is made (a list or the format on a choice, a stepper on each press, a text field on
   Enter, when it loses the focus or when the window closes) and applies to videos that have not
   started; a number out of its range, a glossary that cannot be read, or a models folder moved
   while a download runs is not saved and says why in red under its field, and a `settings.toml`
   that could not be read is kept as `settings.toml.broken` before the first change is saved.
   Folders show the home as `~`, on one line. Watch folders come with the
   [automation](/documentation/features/automation.md) feature.
6. **Models on first use.** A banner under the toolbar says what is missing ("5 models and 2
   runtime libraries are missing (10.7 GiB)"), with Details… (the Models tab) and Download; while
   they download it shows the bytes on disk, the item now and a bar, with Stop (a stopped file
   resumes next time); then for a moment "All models are on disk." No job starts before then.

## Data

- Settings: `~/.config/tbd-subtitles/settings.toml`.
- Jobs: the work directory of each job ([system overview](/documentation/architecture/system_overview.md#job-work-directory));
  the GUI reads `job.json`, `qc.json`, `output.json`, `review.json`, `report.md` and the stage
  outputs, and listens to the runner's progress events. Each finished row's verdict and lines to
  check are read from `qc.json` and `review.json` when the window opens and after each run and
  correction of its video.

## Design

A toolbar over two panes: the videos in a sidebar on the left, the selected job (a header over
its cards, report or review, or with no video a card to drop or add them) on the right; Settings
in a second window. The owner approved a macOS-like redesign as a clickable mockup, built in phases (see
the [roadmap](/documentation/roadmap.md#m2--desktop-gui)). Its look is in place: the mockup's
light and dark palettes (a macOS blue accent, greys, green, orange and red whose text reads at a
contrast of at least 4.5), Adwaita Sans from the system in regular, semibold and bold and Adwaita
Mono for monospace text (egui's fonts when they are missing), Phosphor icons, title 22, headline
15, body 13 and caption 11, controls 28 px high with radius 6, cards and windows with radius 10,
1 px borders and one soft shadow, and selections in a light accent tint with accent text. The
detail pane is built as mocked: the header with the 22 px title, cards in a column at most 800 px
wide, a segmented control with a count, disclosures, pills, thick and thin progress bars and the
stage marks, and the Overview's cards. Check Lines is built as mocked too: the 330 px list, the
editor over the grouped grey, switches, the clip's still frame and a painted timeline. Two things
differ: the playhead follows the time since the sound started, so it may lead the audio by tens of
milliseconds, and Looks Right re-times the line on its own, as its hover text says. The Settings
window is built as mocked: the tab bar of icons over names, forms with right-aligned labels, grey
help and red errors, steppers, the models table and the footer; the banner under the toolbar is
too, and says "models and runtime libraries" when both are missing. The
window follows the desktop's light or dark colour scheme as KDE sets it, through the desktop
portal, and switches when it changes; KDE's accent colour is not followed. Code layout follows the TBD-Reforger desktop-app pattern: one folder per feature with
`models/`, `services/` and `ui/`, where the UI draws from a borrowed view and returns events that
the application applies after the frame. Its architecture tests
(`apps/tbd_subtitles/src/tests/architecture_rules.rs`) hold the pattern here; no ticket system comes
from that project. The renderer is glow (OpenGL), because wgpu fails to create a surface on the
owner's Wayland desktop. The window runs under X11 (XWayland), where files can be dropped onto it
and a second window can be placed over it. An ignored test renders the real window offscreen at
1280 by 800, light and dark, from a copy of real work folders
(`apps/tbd_subtitles/src/application/tests/window_snapshots.rs`), so each phase of the redesign
can be compared with the mockup.

## Open work

- Milestone M2 in the [roadmap](/documentation/roadmap.md#m2--desktop-gui), which ends with the
  batch of Dressrosa 12–48 run from the window's queue.

## Decisions

- eframe over iced or Slint: the owner already uses it, it is the most active Rust GUI toolkit, and
  it draws the clip frames FFmpeg decodes
  ([clips play through FFmpeg](/documentation/decisions/desktop_gui.md#2026-09-26--clips-play-through-ffmpeg-not-libmpv)).
- The desktop portal for choosers and for opening a video
  ([the desktop portal](/documentation/decisions/desktop_gui.md#2026-09-26--the-desktop-portal-chooses-files-and-opens-videos)),
  and a review step for the owner's corrections
  ([review step](/documentation/decisions/desktop_gui.md#2026-09-26--the-owners-corrections-are-timed-by-a-review-step)).
- The desktop's colour scheme through the portal, Adwaita Sans from the system, icons from
  egui-phosphor, and X11 over Wayland
  ([the window's look](/documentation/decisions/desktop_gui.md#2026-09-28--the-window-follows-the-desktops-colour-scheme-in-adwaita-sans-under-x11)).
- A job keeps its own settings once it has started
  ([a job keeps its settings](/documentation/decisions/desktop_gui.md#2026-09-28--a-job-keeps-its-own-settings-once-it-has-started)).
- Settings in their own window
  ([Settings window](/documentation/decisions/desktop_gui.md#2026-09-28--settings-open-in-a-window-of-their-own)),
  correction runs on their video's row
  ([one row per video](/documentation/decisions/desktop_gui.md#2026-09-28--correction-runs-show-in-their-videos-row)),
  and Try Again that starts at once
  ([Try Again](/documentation/decisions/desktop_gui.md#2026-09-28--try-again-resumes-after-the-kept-steps-and-starts-at-once-when-nothing-runs)),
  which reruns adjudication for a failed language-model call
  ([a failed call is retried](/documentation/decisions/desktop_gui.md#2026-09-28--a-failed-language-model-call-is-retried-by-running-adjudication-again)).
- Settings apply as they change, with no Save
  ([settings apply as they change](/documentation/decisions/desktop_gui.md#2026-09-28--settings-apply-as-they-change)).
