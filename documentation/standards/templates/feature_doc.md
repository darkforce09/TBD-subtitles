**Status:** live

# Template: feature document

**When to use:** the documentation of one user-facing feature, the layer above the code READMEs:
what the user sees and does, the data it uses, its design, its open work and the decisions behind
it. Feature documents live in `documentation/features/`, one file each, named in snake_case after
the feature (`gui.md`, `automation.md`). The subtitle pipeline is architecture, not a feature, and
lives in `documentation/architecture/`. The
[documentation standards](/documentation/standards/documentation_standards.md) fix the sections;
the [README standard](/documentation/standards/readme_standard.md) holds the writing rules a
feature document shares with READMEs.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. The sections
come in this order, spelled this way; finer structure goes into `###` headings inside them.

````markdown
**Status:** live

# <Feature name, in plain words>

<One to three sentences: what the feature does and for whom. A feature not built yet, or built
only in part, says so here and names what exists.>

## Where it lives

- Code: <repository-root links to the code folders, and the files that matter most; "(planned)"
  on a folder that does not exist yet, named in plain words rather than as a path>
- Entry: <the subcommand, window or launcher that starts the feature, with the file that declares
  it>
- Related: <links to the feature documents this one feeds or hands off to>

## Behaviour

<What the user sees and does, as the code behaves now: the flows as numbered steps, the rules and
limits, and the reasons behind them. Interface text is quoted as the code writes it. What is
planned but not built is marked so, step by step.>

## Data

- <a file, setting or work-directory output the feature reads or writes>: <its path, its format
  or type, and what the feature does with it>

## Design

<The layout as built and as planned, the code pattern it follows, and each difference between the
two.>

## Open work

- [<milestone and title, as the roadmap heading spells them>](/documentation/roadmap.md#<heading anchor>):
  <each unticked item of that milestone that changes this feature, in a few words>

## Decisions

- <the decision in one sentence>: <why, and what it rules out>; <a link to its decision log entry
  when it has one>
````

Open work lists only unticked roadmap items; there are no tickets. It says "None." when nothing
is open. A gap no roadmap item covers goes to the owner, never into Open work, because only the
owner adds work to the roadmap. A shipped item's lasting knowledge moves into Behaviour, Data or
Decisions. The feature document and the READMEs of the code folders it covers split the facts:
the READMEs hold what the code declares (modules, surface, commands, boundaries), and the feature
document holds the flows, rules and reasons across them. A feature document stays within 500 lines
(`cargo gates markdown-placement`) and is split by topic into a folder with a README index when it
grows past that.

## Worked sample

A shortened excerpt of `documentation/features/gui.md`, checked against the app's window code in
`apps/tbd_subtitles/src/` and the roadmap: each section keeps its first points and leaves the rest
to the real document. The sample sits in a fenced block, so no gate reads it; the feature's own
document is written from the same code and may differ.

````markdown
**Status:** live

# Desktop GUI

The window the owner uses to make subtitles without a terminal: queue videos, run them one at a
time, watch each job's progress and time left, read whether its subtitles pass the quality check,
and fix dialogue and visible Japanese writing. It keeps the owner's approved macOS-like design:
a toolbar, a video sidebar, Overview, Check Lines and Check Text, and a separate Settings window.

## Where it lives

- Code: `apps/tbd_subtitles/`, built with eframe (egui) on the glow renderer. The shell (state,
  frame, actions, shortcuts, the Settings and log windows) is `apps/tbd_subtitles/src/application/`;
  one feature folder each holds the queue (`apps/tbd_subtitles/src/job_queue/`), the report
  (`apps/tbd_subtitles/src/job_report/`), the line and text reviews
  (`apps/tbd_subtitles/src/line_review/`, `apps/tbd_subtitles/src/text_review/`), the log window
  (`apps/tbd_subtitles/src/log_console/`) and the settings (`apps/tbd_subtitles/src/settings/`).
- Entry: `tbd-subtitles gui [VIDEO]...`, or the binary with no subcommand, with or without videos,
  declared in `apps/tbd_subtitles/src/cli/mod.rs`; it runs on the host under X11.
- Related: [automation](/documentation/features/automation.md) feeds the same queue: Dolphin's
  entry, watch folders, and later launches.

## Behaviour

1. **Window.** It opens at 1280 by 800 and can shrink to 1100 by 700, the least that fits the
   sidebar, the line list and the line editor side by side.
2. **Queue.** One job runs at a time and its GPU stages run one after another. Beside it, up to
   four correction runs of different videos run at once, each on a review lane of its own. A job
   takes the settings saved now until it first starts, and its own from then on; the queue is
   kept across windows.
3. **Try Again** puts a failed or cancelled job first in line and resumes after the steps it kept.
4. **Busy.** A job whose `job.redb` another process owns reads "Busy · process 4242 runs this
   video", not failed; it waits again once that process ends.

## Data

- Settings: `~/.config/tbd-subtitles/settings.toml`, read when the window opens and written on
  each change; a broken one is kept as `settings.toml.broken`.
- The queue: `~/.local/share/tbd-subtitles/queue.json`, written on each change and read when the
  window opens.
- Jobs: each job's [work directory](/documentation/glossary.md#work-directory) and its database,
  `job.redb`, read through this process's one handle: the job record and step records for
  progress and the time left, `outputs/qc` and `report.md` for the Overview; line review writes
  `corrections/lines` and Check Text `corrections/text`.

## Design

One folder per feature with `models/`, `services/` and `ui/`: the UI draws from a borrowed view
and returns events that the application applies after the frame, so nothing changes state while a
frame is drawn. The architecture tests (`apps/tbd_subtitles/src/tests/architecture_rules.rs`) hold
the pattern. The renderer is glow (OpenGL), because wgpu fails to create a surface on the owner's
Wayland desktop.

## Open work

- M2 and [M3](/documentation/roadmap.md#m3--automation) are complete.
  [M4](/documentation/roadmap.md#m4--japanese-on-screen-text) still requires annotated pilots,
  resource benchmarks, full GUI/playback checks and owner acceptance;
  [M5](/documentation/roadmap.md#m5--in-place-on-screen-text) its playback checks and the owner's
  acceptance.

## Decisions

- eframe over iced or Slint, and clips through FFmpeg ([clips play through FFmpeg](/documentation/decisions/desktop_gui.md#2026-09-26--clips-play-through-ffmpeg-not-libmpv)).
- One GPU worker at a time on the machine ([one GPU worker](/documentation/decisions/desktop_gui.md#2026-09-26--one-gpu-worker-at-a-time-on-the-machine)).
- A job keeps its own settings once it has started ([a job keeps its settings](/documentation/decisions/desktop_gui.md#2026-09-28--a-job-keeps-its-own-settings-once-it-has-started)).
````
