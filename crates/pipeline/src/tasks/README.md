# Step tasks

The body of every step: read its inputs through its `StepIo`, call the stage, write its outputs
through it. The same code runs inside the job runner for the in-process steps and inside a worker
process of the app binaries for the rest.

## Contents

```text
crates/pipeline/src/tasks/
├── onscreen.rs   the visual steps from detection to typesetting and their isolated model workers
├── alignment.rs  forced alignment: Parakeet-CTC and CTC Viterbi over the vocal stem, block by block
├── io.rs         `StepIo`: a task's stored inputs and outputs, in the runner and in a worker
├── layout.rs     cue building at the frame rate, the quality check, the subtitle file beside the video
├── localized.rs  the localized video: approved patches blended over every frame and encoded
├── llm.rs        adjudication and re-adjudication through `claude -p`, several processes at once
├── media.rs      probe and decode, the shot scan, and vocal separation with the chosen separator
├── mod.rs        `Job`, `TaskReport`, the dispatcher `run`, and `in_process` and `worker_main`
├── replace.rs    stroke masks, inpainting in its ONNX Runtime worker, and lettering composition
├── review.rs     the corrections timed again, each alone, on the CPU; other lines kept
├── rows.rs       `RowStream`: the per-frame rows a worker reads off its stdin one at a time
├── sounds.rs     sound events with CED over both stems, and the sound cues the language model picks
├── speech.rs     voice activity and chunk plan, Parakeet and Whisper, the diff sheet, re-decodes
├── verify.rs     the read-back check: PP-OCRv5 in its ONNX Runtime worker approves each lettering
└── tests/        unit tests for the alignment's inputs, review, the check, the visual steps, the localized subtitles and video, `StepIo` and its row stream
```

## How it works

```text
runner ──▶ in_process(step, StepIo::in_process(store)) ─┐
                                                        ├─▶ run(step, job, io, progress) ──▶ tasks ──▶ stages
worker ──▶ worker_main(step) ── StepIo::over_stdin(step, stdin) ┘
```

Every task is `fn(job: &Job, io: &mut StepIo, progress: StepProgress) -> Result<TaskReport>`.
`StepIo::get(step, part)` is the checked, deserialised document of a step the task reads, and a
missing one is an error naming its key; `view` reads one in place; `probe`, `job_record`,
`corrections` and `text_corrections` are the common reads (no corrections is none).
`StepIo::put(step, part, value)` keeps a document the task writes, refused when its key has no
record kind, and `put_frame(table, occurrence, frame, value)` a `frames` or `readings` row, sent
as it is made. `frame_rows::<T>(table, f)` walks every row of a per-frame table once, in key order,
each checked and read in place, one at a time. In the
runner (`StepIo::in_process`) reads come from one snapshot of the job's store and writes go into
the step's pending write, which `into_outputs` hands the runner to commit with the step's record;
in a worker (`StepIo::over_stdin`) the documents come from the `Input` frames the runner sent down
stdin (`graph::reads`), read before the task starts, the rows of the per-frame tables the step
reads (`graph::reads_rows`) follow them on the pipe and are read only as `frame_rows` asks
(`rows.rs`), and writes go up the channel as `Output` frames; `finish_inputs` drains what the task
did not read before `Done`. The stroke masks write one `frames` row per frame; composition, the
read-back check and the localized video fold those rows into the shift of each frame
(`replace::motion`); the check writes one `readings` row per frame it read. Every task, from the probe to
the localized video, reads and writes its documents only through it: the owner's text corrections
come from `corrections/text`, the typeset ASS events are `outputs/text_typeset/ass`, the diff sheet
also writes `sheet.txt`, the sheet for reading, and the output writes the subtitle files beside the
video.

A `Job` is the work directory and its record; in a worker `Job::received` takes the record from
the `meta/job_record` input. `Job::models` is the models folder the record names, else the
default. `run` sends each step to its task, which returns a `TaskReport` of load time, processing
time and notes. `in_process` resets this process's peak RAM, runs the task on its `StepIo` and
returns its `StepMeasure`. `worker_main` first installs the worker channel
(`worker_channel::worker`), which keeps a private copy of the stdout pipe for frames and points
descriptor 1 at stderr before any native library loads, then reads the step's inputs from stdin.
It refuses a step placed in the other binary, sends each advance as
a `Progress` frame, and at the end sends the load time, processing time, peak RAM, peak child RAM
and notes as an rkyv archive of `WorkerMeasure` in a `Measure` frame, then `Done`; any error,
the placement check's included, goes out as a `Failed` frame with its text before it is returned.
`crate::workers` reads the frames. The language
model steps share one factory of `ClaudeCli` backends, each running in the job's empty
`claude-cwd/`. The Whisper steps load a model only with the `crispasr` feature; without it they
fail and name `tbd-subtitles-ggml`. The output task writes the job's format (SRT, WebVTT or ASS),
moves the file of another format it wrote last time into `backup/`, and stores both as
`outputs/output` (`OutputRecord`), which its next run reads; with on-screen text on, it appends the
typeset events, `outputs/text_typeset/ass`. With the localized video on, it also writes
`<video>.localized.ass`: the dialogue and sound cues alone, no on-screen events, with each cue that
would cover English lettered into the video (every sampled frame of each baked occurrence, from
`outputs/text_verify` and `outputs/text_typeset`, on the ASS canvas and grown by 12
pixels) moved to the top by `subtitle_formats::writers::ass::write_with`. The read-back check
opens PP-OCRv5 only when composition baked something, reads each baked occurrence back through
`stages::onscreen_text::replace::verify` and stores `outputs/text_verify`, which the output and
the localized video read, and one `readings` row per frame it read, which Check Text reads.
The quality check settles the findings of every corrected line;
a Fix It change the owner has not checked has its words held again against every hypothesis, the
re-decodes included, and the summary counts the owner's lines and Fix It's apart. The alignment
and review tasks read both engines' transcripts for `sheet::heard_spans`, so a line is aligned
where any engine heard its words; transcripts that cut other utterances than the sheet holds
leave every line in the backbone's windows. The long tasks
report progress: probe and decode and separation in seconds of audio, the language-model tasks
in batches.

## Boundaries

- Depends on: `stages` (every stage module), `inference` (the ONNX models, CrispASR Whisper, the
  `claude` CLI backend and the model store), `media_io`, `subtitle_formats` (the cue track and the
  subtitle writers), `job_model`, `worker_channel` (the worker's frames), `rkyv` (the measure's
  archive), `crate::graph`, `crate::measure::memory`, `crate::work_dir` (the store and its keys)
  and `crate::workers::StepWrite` (the pending write).
- Used by: `crate::runner` (`in_process`, `StepIo`); `tools/visual_validation/` (the same); the
  `worker` subcommands in
  `apps/tbd_subtitles/src/cli/worker_command.rs`, `apps/tbd_subtitles_ggml/src/main.rs` and
  `apps/tbd_subtitles_llm/src/main.rs` (`worker_main`).
- Rules:
  - a task's stored outputs are committed with its step's record or not at all, and nothing it
    puts is visible before (`an_in_process_step_reads_the_store_and_commits_its_output_with_its_record`
    in `tests/io.rs`); a file it writes goes through `work_dir`'s part files;
  - a worker's inputs arrive down its stdin and its outputs as frames, the same task code as in
    the runner (`a_worker_reads_its_inputs_from_its_stdin_and_sends_its_output_as_frames`), and a
    missing input is an error naming its key (`a_missing_input_is_an_error_naming_its_key`);
  - a per-frame table reaches a task one row at a time, in key order, in the runner and in a
    worker alike, and a worker drains the rows it did not read
    (`a_step_reads_every_frame_row_in_key_order_in_the_runner_and_in_a_worker`,
    `a_worker_that_reads_no_rows_drains_them_so_the_runner_ends_cleanly` in `tests/io.rs`);
  - the subtitle files beside the video are the only files written outside the work directory,
    and a replaced one is kept in the job's `backup/` (`layout.rs`);
  - the localized subtitle file carries no on-screen events and moves a cue over lettered English
    to the top (`the_localized_subtitles_carry_dialogue_alone_moved_above_lettered_writing` in
    `tests/layout.rs`);
  - a worker of the alignment receives every document the task reads, and the task needs each
    one (`the_worker_inputs_of_the_alignment_hold_everything_the_task_reads` in
    `tests/alignment.rs`);
  - the video is only read, and audio is streamed, never held whole (`media.rs`);
  - every crop, still, mask, plate, patch and preview a visual document names, and the localized
    video, is synced on disk before the document is put (`stages::onscreen_text::png`,
    `install` in `localized.rs`);
  - a visual step's worker needs no input beyond `graph::reads`
    (`the_review_reads_the_translation_the_probe_the_shots_and_the_text_corrections` in
    `tests/onscreen.rs`, `a_composition_with_nothing_baked_passes_through_the_check_unchanged` in
    `tests/verify.rs`);
  - a Fix It change the owner has not checked is checked again for words no engine heard
    (`owner_lines_are_settled_and_fix_it_lines_are_checked_again` in `tests/layout.rs`);
  - a binary without `crispasr` refuses a Whisper step instead of skipping it (`speech.rs`);
  - a worker runs only the steps `graph::placement` gives its binary (`mod.rs`);
  - a worker installs its channel before anything else and ends with `Measure` then `Done`, or
    with `Failed` (`worker_main` in `mod.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does, in order.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why the Whisper tasks build only into `tbd-subtitles-ggml`.
