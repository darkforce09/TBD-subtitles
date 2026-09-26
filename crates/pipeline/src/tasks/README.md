# Step tasks

The body of every step: read its inputs from the work directory, call the stage, write its
outputs. The same code runs inside the job runner for the in-process steps and inside a worker
process of either app binary for the rest.

## Contents

```text
crates/pipeline/src/tasks/
├── alignment.rs  forced alignment: Parakeet-CTC and CTC Viterbi over the vocal stem, block by block
├── layout.rs     cue building at the frame rate, the quality check, the subtitle file beside the video
├── llm.rs        adjudication and re-adjudication through `claude -p`, several processes at once
├── media.rs      probe and decode, the shot scan, and vocal separation with the chosen separator
├── mod.rs        `Job`, `TaskReport`, the dispatcher `run`, and `in_process` and `worker_main`
├── sounds.rs     sound events with CED over both stems, and the sound cues the language model picks
└── speech.rs     voice activity and chunk plan, Parakeet and Whisper, the diff sheet, re-decodes
```

## How it works

```text
runner ──▶ in_process(step) ─┐
                             ├─▶ run(step, job, progress) ──▶ media | speech | sounds | llm
worker ──▶ worker_main(step) ┘                                  | alignment | layout ──▶ stages
```

A `Job` is the work directory and its record; `Job::load` reads `job.json`, so a worker needs only
the job's folder. `Job::models` is the models folder the record names, else the default. `run` sends each step to its task, which returns a `TaskReport` of load time,
processing time and notes. `in_process` resets this process's peak RAM, runs the task and returns
its `StepMeasure`. `worker_main` refuses a step placed in the other binary, prints each advance as
a `progress <done> <total>` line on stdout, and writes the load time, processing time, peak RAM,
peak child RAM and notes to `steps/<step>.worker.json`, which `crate::workers` reads. The language
model steps share one factory of `ClaudeCli` backends, each running in the job's empty
`claude-cwd/`. The Whisper steps load a model only with the `crispasr` feature; without it they
fail and name `tbd-subtitles-ggml`. The output task writes the job's format (SRT, WebVTT or ASS),
moves the file of another format it wrote last time into `backup/`, and records both in
`output.json` (`OutputRecord`). The long tasks report progress: probe and decode and separation in
seconds of audio, the language-model tasks in batches.

## Boundaries

- Depends on: `stages` (every stage module), `inference` (the ONNX models, CrispASR Whisper, the
  `claude` CLI backend and the model store), `media_io`, `subtitle_formats` (the cue track and the
  subtitle writers), `job_model`, `crate::graph`, `crate::measure::memory` and `crate::work_dir`.
- Used by: `crate::runner` (`in_process`); the `worker` subcommands in
  `apps/tbd_subtitles/src/cli/worker_command.rs` and `apps/tbd_subtitles_ggml/src/main.rs`
  (`worker_main`).
- Rules:
  - a task writes its outputs completely or not at all, through `work_dir`'s part files;
  - the subtitle file beside the video is the only file written outside the work directory, and a
    replaced one is kept in the job's `backup/` (`layout.rs`);
  - the video is only read, and audio is streamed, never held whole (`media.rs`);
  - a binary without `crispasr` refuses a Whisper step instead of skipping it (`speech.rs`);
  - a worker runs only the steps `graph::placement` gives its binary (`mod.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does, in order.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why the Whisper tasks build only into `tbd-subtitles-ggml`.
