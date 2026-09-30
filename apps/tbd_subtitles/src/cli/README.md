# Command line

The subcommands of the `tbd-subtitles` binary: `gui` (or videos with no subcommand) opens the
window, or hands its videos to the window already open, `process` runs a job for each video from
probe to subtitle file without a window, or with `--enqueue` queues the videos in the window,
`fix` runs
[Fix It](/documentation/glossary.md#fix-it) on a finished video and the correction run after it,
`dump` prints the rows of a job's database as JSON, and `worker` runs one step of a job in its own
[worker process](/documentation/glossary.md#worker-process). People run the first four; `worker`
is the entry point the job runner starts.

## Contents

```text
apps/tbd_subtitles/src/cli/
├── dump_command.rs     the `dump` job resolution, the job database opened if free, its rows as JSON
├── fix_command.rs      the `fix` options over the settings file, Fix It, the correction run, the printout
├── mod.rs              `Cli` and its subcommands in clap, and the dispatch to each runner
├── process_command.rs  the `process` options over the settings file, folders expanded, the run, its printout and exit code
├── tests/              parsing, the window's starts, the process and fix settings, folders, exit codes, the worker step check, the refusals and dump
├── window_command.rs   videos alone, `gui` and `process --enqueue`: the single instance, then the window or a hand-off
└── worker_command.rs   the `worker` runner and its step check: every step but the Whisper ones
```

## How it works

`run` parses the process arguments with clap's derive API. The starts that concern the window
(videos with no subcommand, as Gear Lever's desktop entry runs the AppImage; `gui`; `process
--enqueue`, as Dolphin's "Generate subtitles" runs it) go to `window_command`, which claims the
single instance (`crate::core::single_instance`, in `$XDG_RUNTIME_DIR/tbd-subtitles`) before
logging starts, so only the window that opens empties the window's log file. The first start
installs logging to the log file as well and opens the window (`crate::application::launch`),
serving the later starts' hand-offs to it; a later start logs to stderr only, hands its videos
to the open window (waiting up to 5 s for its answer) and exits 0, or 1 with the reason when the
window did not take them, never opening a second window. A claim that fails with an error is
logged and the window opens without the single instance. Every other subcommand installs
logging to stderr and `dispatch` sends it to its runner, never touching the instance lock. Every
runner returns an `anyhow::Result` of the exit code, which `apps/tbd_subtitles/src/main.rs`
returns, or 1 with the error chain on stderr. clap itself prints the usage and exits 2 on a
usage error, including a step refused by `worker_command::parse_step`, an unknown `--rerun`
step, or a job option beside `--enqueue`.

```text
tbd-subtitles [COMMAND] ──▶ Cli::parse
   [VIDEOS]... | gui [VIDEOS]...     ──▶ window_command::run: claim ──▶ launch, or hand off (raise)
   process --enqueue <PATHS>...      ──▶ window_command::run: claim ──▶ launch minimized and start,
                                         or hand off (start)
   process <PATHS>... [OPTIONS]      ──▶ dispatch ──▶ process_command::run ──▶ pipeline::run_job, per video
   fix <VIDEO> [OPTIONS]             ──▶ dispatch ──▶ fix_command::run ──▶ pipeline::fix_it::fix_video, run_job
   dump <JOB> <TABLE> [KEY]          ──▶ dispatch ──▶ dump_command::run ──▶ JobStore::open_existing, kinds
   worker <STEP> <JOB_DIR>           ──▶ dispatch ──▶ worker_command::run ──▶ pipeline::tasks::worker_main
```

`process_command::merged` puts the options over the settings file (`--settings`, else
`~/.config/tbd-subtitles/settings.toml`, read by `crate::settings::services::settings_file`; no
file means the defaults), and `process_command::settings` turns the result into a `JobSettings`
through `crate::settings::services::job_settings`: the glossary (the built-in
`stages::adjudication::glossary::one_piece`, none, or a JSON file of names), the separator, the
Whisper model, the shot-cut score, the language model and the output format, plus the audio
track, which only the command line names. `run` expands each folder into every video under it
without a subtitle file (`job_queue::services::video_files::videos_under`), keeps each video
once, checks every path before the first job starts, finds the job runner's two binaries beside the running one
(`Binaries::beside_current_exe`), runs the jobs in order and stops at the first that fails. Each
[step](/documentation/architecture/pipeline.md) prints one line to stderr as it starts (`>`), is
skipped as still valid (`=`), advances, and finishes (`✓`, with its time and peak RAM and VRAM).
Once every job ran, `process_command::verdict` turns their quality checks into the exit code: 0
when each passed, else 2 with a line naming the videos that failed it.

## Commands

Each runs as `cargo run -p tbd_subtitles -- <arguments>` from the repository root, or as
`tbd-subtitles <arguments>` once built. `--help` on the binary or a subcommand prints its usage;
`--version` prints the version.

### gui

- Synopsis: `tbd-subtitles gui [VIDEOS]...`, or `tbd-subtitles [VIDEOS]...` with no subcommand.
- Does: opens the desktop window with the given videos added to the kept queue, and returns when
  the window closes. The window runs the queued jobs one at a time. When the window is open
  already, it adds the videos to that window's queue and brings it forward instead, and returns
  at once.
- Exit codes: 0 when the window closes, or once the open window took the videos; 1 when it
  cannot open, or the open window did not answer within 5 s; 2 on a usage error.
- Example: `cargo run -p tbd_subtitles -- episode_01.mkv episode_02.mkv`

### process

- Synopsis: `tbd-subtitles process <PATHS>... [--settings <FILE>] [--work-root <DIR>] [--models-dir <DIR>] [--glossary <one_piece|none|FILE>] [--audio-track <N>] [--separator <roformer|mdx-net>] [--whisper <large-v3|large-v3-turbo>] [--cut-score <SCORE>] [--llm-model <MODEL>] [--format <srt|vtt|ass>] [--rerun <STEP>]...`,
  or `tbd-subtitles process --enqueue <PATHS>...`
- Does: runs one job per video, in order: a video named always, and a folder as every video
  anywhere under it that has no subtitle file yet (hidden folders and downloads still in progress
  left out), each video once. It prints where each wrote its subtitles
  (`<video base name>.srt`, `.vtt` or `.ass` beside the video), its report (`report.md` in the
  job's [work directory](/documentation/glossary.md#work-directory)) and a quality line (cues,
  findings, the share within 20 characters per second, and whether the job passes the quality
  check or why not). Each job resumes from the steps whose output is still valid. Every option but
  `--audio-track` and `--rerun` wins over the same setting in the settings file; without either,
  the defaults are work directories under `tbd-subtitles/work/` in the data folder, models under
  `tbd-subtitles/models/`, the `one_piece` glossary, the English audio track, `roformer`,
  `large-v3`, a cut score of 20, the `sonnet` model and SRT. `--rerun` names a step to run
  again even when its output is valid, and may repeat. With `--enqueue` it runs nothing itself:
  the videos (folders expanded the same way) go to the window's queue and the queue starts, unless
  the owner pressed Pause in that window; the window already open takes them without coming
  forward, or the window opens minimized. `--enqueue` takes none of the other options, since the
  window's jobs take its saved settings; Dolphin's "Generate subtitles" runs it.
- Exit codes: 0 every job finished and passed the quality check (with `--enqueue`: the window
  took the videos, or the window it opened closed); 2 every job finished but at least one failed
  the quality check, named on the last line; 1 a missing or unreadable video, a path that is not
  a file or a folder, a folder with no video without subtitles, an unreadable or invalid settings
  file, an unreadable glossary file, a model missing from the models folder (named, with where to
  download it), a job whose step failed, with the reason, or an open window that did not take the
  videos; 2 also on a usage error, including no video, a `--rerun` value that is no step and an
  option beside `--enqueue`.
- Example: `distrobox-host-exec target/release/tbd-subtitles process "Dressrosa 08.mp4" --rerun cues`

### fix

- Synopsis: `tbd-subtitles fix <VIDEO> [--settings <FILE>] [--work-root <DIR>] [--model <MODEL>] [--processes <N>]`
- Does: runs Fix It on the video's finished job with the Fix It model (`opus` unless the settings
  or `--model` say another), printing each pass as it goes, then the brief (show, episode, cast),
  each line asked about with its verdict, its change and any refused proposal, and the calls with
  their cost. When it changed a line, it runs the job again with the job's own settings, so the
  review step and the steps after it put the changes into the subtitle file, and prints the
  quality line as `process` does. `--processes` sets how many `claude` calls run at once; a call
  the provider answers as busy (rate limit, overloaded) is asked again after 30, 60 and 120 s.
- Exit codes: 0 done, changed or not; 1 a missing video, an unreadable settings file, a job that
  is not finished or whose corrections are not in its subtitles yet, a brief that could not be
  made, or a correction run that failed; 2 on a usage error, including no video.
- Example: `distrobox-host-exec target/release/tbd-subtitles fix "[Muhn Pace] Dressrosa 12.mp4"`

### dump

- Synopsis: `tbd-subtitles dump <JOB_OR_VIDEO> <TABLE> [KEY] [--settings <FILE>] [--work-root <DIR>]`
- Does: finds the job `<JOB_OR_VIDEO>` names: a folder that holds `job.redb`, else a job folder
  under the work root (from `--work-root`, else the settings file as `process` reads it, else the
  default), else a video, whose job folder is named from its canonical path as the runner names
  it. It opens the job's `job.redb` only when no other process owns it
  (`pipeline::work_dir::JobStore::open_existing`, which creates nothing) and reads `<TABLE>`
  (`meta`, `step_records`, `outputs`, `corrections`, `frames` or `readings`). With `[KEY]` (a
  name such as `layout` or `cues/dropped_sounds`, or `<occurrence>/<frame>` in `frames` and
  `readings`) it prints that row's record as pretty JSON through its record kind
  (`pipeline::work_dir::store::kinds`); without it, every row of the table as JSON Lines in key
  order, one `{"key": …, "value": …}` object each, where a row with no record kind has a null
  value and its size in `"bytes"`. It never opens the window.
- Exit codes: 0 printed; 1 no such job, a job with no database, a missing row, or a job another
  process is running ("process N is running this job; dump it after that run ends"); 2 on a usage
  error, including a table that does not exist and a per-frame key without its frame.
- Example: `distrobox-host-exec target/release/tbd-subtitles dump "[Muhn Pace] Dressrosa 11.mp4" meta layout`

### worker

- Synopsis: `tbd-subtitles worker <STEP> <JOB_DIR>`.
- Does: runs `<STEP>` over the job in `<JOB_DIR>` in this process through
  `pipeline::tasks::worker_main`, writing the step's output to the work directory and sending
  its progress, model calls, measure and end or failure to the job runner as frames of the worker
  channel on stdout; anything else printed to stdout goes to stderr. Every step is accepted but
  `asr_whisper` and `redecode_whisper`, which are refused as belonging to `tbd-subtitles-ggml`.
- Exit codes: 0 the step finished; 1 the job cannot be loaded or the step failed; 2 on a usage
  error, including a Whisper step or a name that is no step.
- Example: `target/release/tbd-subtitles worker separation ~/.local/share/tbd-subtitles/work/<job>`

## Boundaries

- Depends on: `crate::application` (`launch`, `Launch`, `HandOffs`); `crate::core::logging`;
  `crate::core::single_instance` (the claim, the serving thread, the hand-off);
  `crate::job_queue::services::video_files` (a folder's videos); `pipeline` (`run_job`, `JobOptions`,
  `progress::Progress`, `workers::Binaries`, `work_dir::default_root`, `graph::placement`,
  `tasks::worker_main`, `fix_it::fix_video`, `work_dir::{JobStore, WorkDir, job_id}`,
  `work_dir::store::{kind, kinds}`) from `crates/pipeline/`; `worker_channel::address::{Table,
  Key}`; `serde_json`; `job_model::StepName` and `job_model::job` from
  `crates/job_model/`; `crate::settings::{models, services}` (the settings file and the job
  settings it makes); `clap` and `anyhow`.
- Used by: `apps/tbd_subtitles/src/main.rs`, which calls `run`; the job runner in
  `crates/pipeline/`, which starts `worker`.
- Rules:
  - running with no subcommand opens the window (`no_subcommand_opens_the_window`), and the clap
    definition stays consistent (`the_command_definition_is_consistent`);
  - `gui` takes optional videos and `process` needs at least one
    (`gui_takes_optional_videos`, `process_needs_at_least_one_video`);
  - `process` defaults to the built-in glossary and the measured stack, and every option reaches
    the settings (`process_defaults_to_the_built_in_glossary_and_the_measured_stack`,
    `process_options_reach_the_settings`), and an option wins over the settings file
    (`options_win_over_the_settings_file`);
  - videos with no subcommand, `gui` and `process --enqueue` go to the window and nothing else
    does; the first two raise an open window, `--enqueue` starts its queue and opens a new one
    minimized (`videos_without_a_subcommand_open_the_window_with_them`,
    `gui_and_enqueue_concern_the_window_and_the_rest_do_not`), and `--enqueue` takes no job
    option (`enqueue_takes_no_job_option`, `process_run_refuses_enqueue`);
  - a folder gives every video under it without subtitles, a video named is always processed,
    and a folder with none fails naming it
    (`a_folder_gives_every_video_under_it_without_subtitles`); the exit code is 0 when every job
    passed the quality check and 2, naming the others, when one did not
    (`the_exit_code_says_whether_every_job_passed_the_quality_check`);
  - `worker` refuses the Whisper steps by naming `tbd-subtitles-ggml`
    (`worker_takes_main_binary_steps_only`);
  - `process` names a missing video, and `worker` fails without a job; neither reports success
    it has not earned (`process_refuses_a_missing_video_by_name`, `a_worker_without_a_job_fails`);
  - `dump` takes a job, a table and an optional key in its table's form, finds a job by its
    folder, its name or its video, and prints one row pretty or a table as JSON Lines, a row
    without a record kind by its size (`dump_takes_a_job_a_table_and_an_optional_key`,
    `a_key_takes_the_form_of_its_table`, `a_job_is_found_by_its_folder_its_name_or_its_video`,
    `a_store_dumps_one_row_pretty_or_a_table_as_json_lines` in `tests/dump_command.rs`);
  - the rest in `tests/cli.rs`; the step names are an interface the job runner calls by name, and
    `StepName` owns them.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — the steps a `process` run goes through.
- [System overview](/documentation/architecture/system_overview.md) — the `gui`, `process` and
  `worker` processes.
- [Automation](/documentation/features/automation.md) — the Dolphin entry that runs
  `process --enqueue`, and the watch folders.
