# Step tasks

The body of every step: read its inputs from the work directory, call the stage, write its
outputs. The same code runs inside the job runner for the in-process steps and inside a worker
process of either app binary for the rest.

## Contents

```text
crates/pipeline/src/tasks/
├── onscreen.rs   the visual steps from detection to typesetting and their isolated model workers
├── alignment.rs  forced alignment: Parakeet-CTC and CTC Viterbi over the vocal stem, block by block
├── layout.rs     cue building at the frame rate, the quality check, the subtitle file beside the video
├── localized.rs  the localized video: approved patches blended over every frame and encoded
├── llm.rs        adjudication and re-adjudication through `claude -p`, several processes at once
├── media.rs      probe and decode, the shot scan, and vocal separation with the chosen separator
├── mod.rs        `Job`, `TaskReport`, the dispatcher `run`, and `in_process` and `worker_main`
├── replace.rs    stroke masks, inpainting in its ONNX Runtime worker, and lettering composition
├── review.rs     the corrections timed again, each alone, on the CPU; other lines kept
├── sounds.rs     sound events with CED over both stems, and the sound cues the language model picks
├── speech.rs     voice activity and chunk plan, Parakeet and Whisper, the diff sheet, re-decodes
├── verify.rs     the read-back check: PP-OCRv5 in its ONNX Runtime worker approves each lettering
└── tests/        unit tests for review, the check's corrections, the localized subtitles and video
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
`output.json` (`OutputRecord`). With the localized video on, it also writes
`<video>.localized.ass`: the dialogue and sound cues alone, no on-screen events, with each cue that
would cover English lettered into the video (every sampled frame of each baked occurrence, from
`visual/text_verify.json` and `visual/text_typeset.json`, on the ASS canvas and grown by 12
pixels) moved to the top by `subtitle_formats::writers::ass::write_with`. The read-back check
opens PP-OCRv5 only when composition baked something, reads each baked occurrence back through
`stages::onscreen_text::replace::verify` and writes `visual/text_verify.json`, which the output and
the localized video read. The quality check settles the findings of every corrected line;
a Fix It change the owner has not checked has its words held again against every hypothesis, the
re-decodes included, and the summary counts the owner's lines and Fix It's apart. The alignment
and review tasks read both engines' transcripts (Whisper's when it is there) for
`sheet::heard_spans`, so a line is aligned where any engine heard its words; transcripts that cut
other utterances than the sheet holds are an error. The long tasks
report progress: probe and decode and separation in seconds of audio, the language-model tasks
in batches.

## Boundaries

- Depends on: `stages` (every stage module), `inference` (the ONNX models, CrispASR Whisper, the
  `claude` CLI backend and the model store), `media_io`, `subtitle_formats` (the cue track and the
  subtitle writers), `job_model`, `crate::graph`, `crate::measure::memory` and `crate::work_dir`.
- Used by: `crate::runner` (`in_process`); the `worker` subcommands in
  `apps/tbd_subtitles/src/cli/worker_command.rs` and `apps/tbd_subtitles_ggml/src/main.rs`
  (`worker_main`).
- Rules:
  - a task writes its outputs completely or not at all, through `work_dir`'s part files;
  - the subtitle files beside the video are the only files written outside the work directory,
    and a replaced one is kept in the job's `backup/` (`layout.rs`);
  - the localized subtitle file carries no on-screen events and moves a cue over lettered English
    to the top (`the_localized_subtitles_carry_dialogue_alone_moved_above_lettered_writing` in
    `tests/layout.rs`);
  - the video is only read, and audio is streamed, never held whole (`media.rs`);
  - a Fix It change the owner has not checked is checked again for words no engine heard
    (`owner_lines_are_settled_and_fix_it_lines_are_checked_again` in `tests/layout.rs`);
  - a binary without `crispasr` refuses a Whisper step instead of skipping it (`speech.rs`);
  - a worker runs only the steps `graph::placement` gives its binary (`mod.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does, in order.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why the Whisper tasks build only into `tbd-subtitles-ggml`.
