# Step graph

The one table of the pipeline's steps: what each reads, where it runs, whether it loads a model
onto the GPU, which settings it depends on, its revision, how long it may run, which stored values
it reads and which steps depend on it. The run order is `StepName::ALL` in `crates/job_model/src/stage/step_name.rs`.

## Contents

```text
crates/pipeline/src/graph/
├── mod.rs  placement, GPU/runtime needs, inputs, stored reads, dependents, settings, revisions
└── tests/  unit tests for inputs, placements, reads, dependents, settings and revisions
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
Whisper, adjudication and the sound cues, 360 for the first four visual steps, and 60 for the rest.
`reads` lists every stored value a step reads, as the addresses its worker receives down stdin:
the job record, every document (`work_dir::store::keys::output_parts`) of every step it reads, the
line corrections for the review and the quality check, the text corrections for reading,
translation and review, and, for the output and the localized video, their own earlier run's
record; `is_optional_read` marks the corrections and that earlier record, which a job may not
have. `dependents` is every step that reads a step, directly or through others, in run order;
`--rerun` clears them with it. Which documents a step writes is `work_dir::store::keys`, and which
files its rows name is `work_dir::store::files`.

## Boundaries

- Depends on: `job_model` (`StepName`, `JobSettings`), `serde_json`, `worker_channel::address`,
  and `crate::work_dir::store::keys` for the rows a step writes.
- Used by: `crate::runner`, `crate::resume`, `crate::workers` and `crate::tasks`; the `worker`
  subcommands in `apps/tbd_subtitles/src/cli/worker_command.rs` and
  `apps/tbd_subtitles_ggml/src/main.rs`, which check a step's placement.
- Rules:
  - a step reads only steps before it (`every_step_reads_only_earlier_steps` in
    `tests/graph.rs`);
  - every GPU step runs in a worker, and only the Whisper steps run in the ggml binary
    (`gpu_steps_run_in_workers_and_whisper_alone_in_the_ggml_binary`);
  - a step reads the job record and every document of every step it reads, each with a record
    kind (`a_step_reads_the_job_record_and_every_document_of_every_step_it_reads`);
  - a step's dependents are every step that reads it through any path, all after it
    (`a_steps_dependents_are_every_step_that_reads_it_through_any_path`);
  - only the settings a step reads reach its fingerprint
    (`only_the_settings_a_step_reads_reach_its_fingerprint`);
  - alignment and review read both transcripts (`alignment_and_review_read_both_transcripts`);
  - the output reads the composition, for the localized subtitle file's placement; the visual
    review reads the shots; typesetting reads the review alone and leaves no localized events
    (`the_output_reads_the_composition_and_the_text_review_reads_the_shots`);
  - a change to what a step writes raises its revision (the module header;
    `changed_steps_carry_their_revision_and_the_rest_are_at_one`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow and its inputs.
