# Step graph

The one table of the pipeline's steps: what each reads, where it runs, whether it loads a model
onto the GPU, which settings it depends on, its revision, how long it may run, which stored values
it reads, which steps depend on it, and which steps form the visual lane. The run order is `StepName::ALL` in `crates/job_model/src/stage/step_name.rs`.

## Contents

```text
crates/pipeline/src/graph/
├── gpu.rs  each GPU step's memory need, the lazily locking step, and the lane's place in the queue
├── mod.rs  placement, GPU/runtime needs, inputs, stored reads, dependents, settings, revisions, lane
└── tests/  unit tests for inputs, placements, reads, dependents, settings, revisions, lane and GPU
```

## How it works

`placement` puts voice activity, the diff sheet, cue building, the quality check and the output in
the runner's own process; the Whisper steps in a worker of `tbd-subtitles-ggml`
(`Binary::Ggml`); visual translation in `tbd-subtitles-llm` (`Binary::LocalLlm`); visual review in
the runner; every other step in a worker of `tbd-subtitles` (`Binary::Main`). Typesetting uses a
CPU worker so cancellation interrupts long glyph and layout work. `uses_gpu`
marks the steps that load a model onto the GPU, which take the GPU lock and a VRAM monitor;
`loads_onnx_runtime` adds the review step, which runs Parakeet-CTC on the CPU, to the steps that
load ONNX Runtime; `needs_cuda_runtime` is those steps plus the Whisper steps, whose ggml CUDA
backend finds `libcudart` and `libcublas` only on the library path, and names the workers that
get the CUDA runtime's environment; `reads_corrections` names the review step as the step whose
fingerprint covers the owner's line corrections, and `reads_text_corrections` reading,
translation and review, which read the on-screen text corrections.
`settings` returns the part of `JobSettings` a step reads, so a changed cut score reruns cue
building and nothing before it, and a changed output format reruns only the output. `revision`
is 1 except for the explicitly versioned steps in `REVISIONS`; a revision change invalidates
outputs produced under the previous contract. `timeout` is 180 minutes for separation, 120 for
Whisper recognition, adjudication and the sound cues, 360 for every visual step but review and
typesetting (the localized video among them), and 60 for the rest.
`reads` lists every stored value a step reads, as the addresses its worker receives down stdin:
the job record, every document (`work_dir::store::keys::output_parts`) of every step it reads, the
line corrections for the review and the quality check, the text corrections for reading,
translation and review, and, for the output and the localized video, their own earlier run's
record; `is_optional_read` marks the corrections and that earlier record, which a job may not
have. `writes_rows` names the per-frame table a step owns (`frames` for the stroke masks,
`readings` for the read-back check), and `reads_rows` the tables whose rows a step's worker
receives after its documents, one `Input` frame per row (`frames` for composition, the check and
the localized video). `dependents` is every step that reads a step, directly or through others, in run order;
`--rerun` clears them with it. `VISUAL_LANE` is `text_detect`, `text_read` and `text_track`,
which read only the probe, the shot scan and each other, so the runner walks them on a thread of
their own (`in_visual_lane`): from `VISUAL_LANE_STARTS_AT`, `adjudicate`, the first step that
waits on Claude, until `VISUAL_LANE_JOINS_AT`, `text_translate`, the first step that reads one
(`reads_visual_lane`). Which documents a step writes is `work_dir::store::keys`, and which
files its rows name is `work_dir::store::files`.

## Boundaries

- Depends on: `job_model` (`StepName`, `JobSettings`), `serde_json`, `worker_channel::address`,
  and `crate::work_dir::store::keys` for the rows a step writes.
- Used by: `crate::runner`, `crate::resume`, `crate::workers` and `crate::tasks`; the window's
  job queue (`apps/tbd_subtitles/src/job_queue/`) for the visual lane; the `worker`
  subcommands in `apps/tbd_subtitles/src/cli/worker_command.rs`,
  `apps/tbd_subtitles_ggml/src/main.rs` and `apps/tbd_subtitles_llm/src/main.rs`, which check a
  step's placement or name their binary.
- Rules:
  - a step reads only steps before it (`every_step_reads_only_earlier_steps` in
    `tests/graph.rs`);
  - the visual lane reads only the probe, the shot scan and its own earlier steps, starts after
    the shot scan and before its steps at a step none of them reads, and is joined at the first
    step that reads it (`the_visual_lane_reads_only_the_probe_the_shot_scan_and_itself`,
    `the_visual_lane_starts_after_the_shot_scan_and_before_its_steps`,
    `the_visual_lane_joins_at_the_first_step_that_reads_it`);
  - every GPU step runs in a worker, and only the Whisper steps run in the ggml binary
    (`gpu_steps_run_in_workers_and_whisper_alone_in_the_ggml_binary`);
  - a step reads the job record and every document of every step it reads, each with a record
    kind (`a_step_reads_the_job_record_and_every_document_of_every_step_it_reads`);
  - a step's dependents are every step that reads it through any path, all after it
    (`a_steps_dependents_are_every_step_that_reads_it_through_any_path`);
  - each per-frame table has one owner, and a step reads rows only of a step it depends on
    (`the_stroke_masks_own_the_frames_rows_that_composition_and_the_video_read`);
  - only the settings a step reads reach its fingerprint
    (`only_the_settings_a_step_reads_reach_its_fingerprint`);
  - alignment and review read both transcripts (`alignment_and_review_read_both_transcripts`);
  - the output and the localized video read the checked replacements; the visual review reads
    the shots; typesetting reads the review alone and leaves no localized events
    (`the_output_and_the_localized_video_read_the_checked_replacements`);
  - the read-back check runs right after composition, before the outputs, in an OCR worker
    (`the_read_back_check_runs_between_composition_and_the_outputs_in_an_ocr_worker`);
  - typesetting runs in a CPU worker, and only inpainting and encoding of the replacement steps
    take the GPU lock
    (`visual_typesetting_runs_in_a_cancellable_worker_without_a_gpu_runtime_or_lock`,
    `replacement_steps_keep_onnx_runtime_and_the_gpu_lock_to_inpainting_and_encoding`);
  - a change to what a step writes raises its revision (the module header;
    `changed_steps_carry_their_revision_and_the_rest_are_at_one`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow and its inputs.
