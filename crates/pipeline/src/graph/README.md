# Step graph

The one table of the pipeline's steps: what each reads, where it runs, whether it loads a model
onto the GPU, which settings it depends on, its revision, how long it may run and which files it
leaves. The run order is `StepName::ALL` in `crates/job_model/src/stage/step_name.rs`.

## Contents

```text
crates/pipeline/src/graph/
├── mod.rs  placement, GPU/runtime needs, inputs, settings, outputs and artifact validation
└── tests/  unit tests for the order of inputs, the placements, the outputs, the settings and revisions
```

## How it works

`placement` puts voice activity, the diff sheet, cue building, the quality check and the output in
the runner's own process; the Whisper steps in a worker of `tbd-subtitles-ggml`
(`Binary::Ggml`); visual translation in `tbd-subtitles-llm` (`Binary::LocalLlm`); visual review in
the runner; every other step in a worker of `tbd-subtitles` (`Binary::Main`). Typesetting uses a
CPU worker so cancellation interrupts long glyph and layout work. `uses_gpu`
marks the steps that load a model onto the GPU, which take the GPU lock and a VRAM monitor;
`loads_onnx_runtime` adds the review step, which runs Parakeet-CTC on the CPU, to the steps that
get the CUDA runtime's environment, and `reads_corrections` names it as the step whose
fingerprint covers the owner's corrections.
`settings` returns the part of `JobSettings` a step reads, so a changed cut score reruns cue
building and nothing before it, and a changed output format reruns only the output. `revision`
is 1 except for the explicitly versioned steps in `REVISIONS`; a revision change invalidates
outputs produced under the previous contract. `timeout` is 180 minutes for separation, 120 for
Whisper, adjudication and the sound cues, 360 for the first four visual steps, and 60 for the rest.
`outputs` lists the fixed artifacts, including the subtitle beside the video in its effective
format. `artifacts_valid` also streams detection manifests to verify every representative crop
exists as a nonempty file within the work directory. A missing crop reruns its producer and visual
descendants while preserving valid audio stages.

## Boundaries

- Depends on: `job_model` (`StepName`, `JobSettings`), `serde_json`, `crate::work_dir::WorkDir`,
  and `stages::output::subtitle_path` for the output step's file.
- Used by: `crate::runner`, `crate::resume`, `crate::workers` and `crate::tasks`; the `worker`
  subcommands in `apps/tbd_subtitles/src/cli/worker_command.rs` and
  `apps/tbd_subtitles_ggml/src/main.rs`, which check a step's placement.
- Rules:
  - a step reads only steps before it (`every_step_reads_only_earlier_steps` in
    `tests/graph.rs`);
  - every GPU step runs in a worker, and only the Whisper steps run in the ggml binary
    (`gpu_steps_run_in_workers_and_whisper_alone_in_the_ggml_binary`);
  - every step leaves at least one file (`every_step_leaves_at_least_one_file`);
  - only the settings a step reads reach its fingerprint
    (`only_the_settings_a_step_reads_reach_its_fingerprint`);
  - alignment and review read both transcripts (`alignment_and_review_read_both_transcripts`);
  - a change to what a step writes raises its revision (the module header;
    `changed_steps_carry_their_revision_and_the_rest_are_at_one`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow and its inputs.
