# Command line

The subcommands of the `tbd-subtitles` binary: `gui` opens the window, `process` runs a job for
each video from probe to subtitle file without a window, and `worker` runs one step of a job in
its own [worker process](/documentation/glossary.md#worker-process). People run the first two;
`worker` is the entry point the job runner starts.

## Contents

```text
apps/tbd_subtitles/src/cli/
├── mod.rs              `Cli` and its subcommands in clap, and the dispatch to each runner
├── process_command.rs  the `process` options, the job settings they make, the run and its printout
├── tests/              parsing, the process settings, the worker step check and the refusals
└── worker_command.rs   the `worker` runner and its step check: every step but the Whisper ones
```

## How it works

`run` parses the process arguments with clap's derive API and `dispatch` sends each subcommand to
its runner; with no subcommand it opens the window, as a desktop launcher expects. Every runner
returns an `anyhow::Result`, which `apps/tbd_subtitles/src/main.rs` turns into the exit code.
clap itself prints the usage and exits 2 on a usage error, including a step refused by
`worker_command::parse_step` or an unknown `--rerun` step.

```text
tbd-subtitles [COMMAND] ──▶ Cli::parse ──▶ dispatch
   (none) | gui [VIDEOS]...          ──▶ crate::application::launch(videos)
   process <VIDEOS>... [OPTIONS]     ──▶ process_command::run ──▶ pipeline::run_job, per video
   worker <STEP> <JOB_DIR>           ──▶ worker_command::run  ──▶ pipeline::tasks::worker_main
```

`process_command::settings` turns the options into a `JobSettings`: the glossary (the built-in
`stages::adjudication::glossary::one_piece`, none, or a JSON file of names), the audio track, the
separator, the Whisper model, the shot-cut score and the language model. `run` checks every video
before the first job starts, finds the job runner's two binaries beside the running one
(`Binaries::beside_current_exe`), runs the jobs in order and stops at the first that fails. Each
[step](/documentation/architecture/pipeline.md) prints one line to stderr as it starts (`>`), is
skipped as still valid (`=`), advances, and finishes (`✓`, with its time and peak RAM and VRAM).

## Commands

Each runs as `cargo run -p tbd_subtitles -- <arguments>` from the repository root, or as
`tbd-subtitles <arguments>` once built. `--help` on the binary or a subcommand prints its usage;
`--version` prints the version.

### gui

- Synopsis: `tbd-subtitles gui [VIDEOS]...`, or `tbd-subtitles` with no subcommand and no videos.
- Does: opens the desktop window with the given videos in the queue, and returns when the window
  closes. The window runs no job.
- Exit codes: 0 when the window closes; 1 when it cannot open; 2 on a usage error.
- Example: `cargo run -p tbd_subtitles -- gui episode_01.mkv episode_02.mkv`

### process

- Synopsis: `tbd-subtitles process <VIDEOS>... [--work-root <DIR>] [--glossary <one_piece|none|FILE>] [--audio-track <N>] [--separator <roformer|mdx-net>] [--whisper <large-v3|large-v3-turbo>] [--cut-score <SCORE>] [--llm-model <MODEL>] [--rerun <STEP>]...`
- Does: runs one job per video, in order, and prints where each wrote its subtitles
  (`<video base name>.srt` beside the video), its report (`report.md` in the job's
  [work directory](/documentation/glossary.md#work-directory)) and a quality line (cues,
  findings, the share within 20 characters per second, whether the layout rules hold). Each job
  resumes from the steps whose output is still valid. The defaults: work directories under
  `tbd-subtitles/work/` in the data folder, the `one_piece` glossary, the English audio track,
  `roformer`, `large-v3`, a cut score of 20, and the `sonnet` model. `--rerun` names a step to run
  again even when its output is valid, and may repeat.
- Exit codes: 0 every job finished; 1 a missing or unreadable video, a path that is not a file,
  an unreadable glossary file, or a job whose step failed, with the reason; 2 on a usage error,
  including no video and a `--rerun` value that is no step.
- Example: `distrobox-host-exec target/release/tbd-subtitles process "Dressrosa 08.mp4" --rerun cues`

### worker

- Synopsis: `tbd-subtitles worker <STEP> <JOB_DIR>`.
- Does: runs `<STEP>` over the job in `<JOB_DIR>` in this process through
  `pipeline::tasks::worker_main`, writing the step's output and its measure file to the work
  directory and `progress <done> <total>` lines to stdout. Every step is accepted but
  `asr_whisper` and `redecode_whisper`, which are refused as belonging to `tbd-subtitles-ggml`.
- Exit codes: 0 the step finished; 1 the job cannot be loaded or the step failed; 2 on a usage
  error, including a Whisper step or a name that is no step.
- Example: `target/release/tbd-subtitles worker separation ~/.local/share/tbd-subtitles/work/<job>`

## Boundaries

- Depends on: `crate::application::launch`; `pipeline` (`run_job`, `JobOptions`,
  `progress::Progress`, `workers::Binaries`, `work_dir::default_root`, `graph::placement`,
  `tasks::worker_main`) from `crates/pipeline/`; `job_model::StepName` and `job_model::job` from
  `crates/job_model/`; `stages::adjudication::glossary` from `crates/stages/`; `clap` and
  `anyhow`.
- Used by: `apps/tbd_subtitles/src/main.rs`, which calls `run`; the job runner in
  `crates/pipeline/`, which starts `worker`.
- Rules:
  - running with no subcommand opens the window (`no_subcommand_opens_the_window`), and the clap
    definition stays consistent (`the_command_definition_is_consistent`);
  - `gui` takes optional videos and `process` needs at least one
    (`gui_takes_optional_videos`, `process_needs_at_least_one_video`);
  - `process` defaults to the built-in glossary and the measured stack, and every option reaches
    the settings (`process_defaults_to_the_built_in_glossary_and_the_measured_stack`,
    `process_options_reach_the_settings`);
  - `worker` refuses the Whisper steps by naming `tbd-subtitles-ggml`
    (`worker_takes_main_binary_steps_only`);
  - `process` names a missing video, and `worker` fails without a job; neither reports success
    it has not earned (`process_refuses_a_missing_video_by_name`, `a_worker_without_a_job_fails`);
  - all in `tests/cli.rs`; the step names are an interface the job runner calls by name, and
    `StepName` owns them.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — the steps a `process` run goes through.
- [System overview](/documentation/architecture/system_overview.md) — the `gui`, `process` and
  `worker` processes.
- [Automation](/documentation/features/automation.md) — the Dolphin entry and watch folders
  that run `process`.
