# Step graph

The one table of the pipeline's steps: what each reads, where it runs, whether it loads a model
onto the GPU, which settings it depends on, its revision, how long it may run and which files it
leaves. The run order is `StepName::ALL` in `crates/job_model/src/stage/step_name.rs`.

## Contents

```text
crates/pipeline/src/graph/
├── mod.rs  `placement`, `uses_gpu`, `loads_onnx_runtime`, `inputs`, `settings`, `outputs` and more
└── tests/  unit tests for the order of inputs, the placements, the outputs and the settings
```

## How it works

`placement` puts voice activity, the diff sheet, cue building, the quality check and the output in
the runner's own process; the Whisper steps in a worker of `tbd-subtitles-ggml`
(`Binary::Ggml`); every other step in a worker of `tbd-subtitles` (`Binary::Main`). `uses_gpu`
marks the steps that load a model onto the GPU, which take the GPU lock and a VRAM monitor;
`loads_onnx_runtime` adds the review step, which runs Parakeet-CTC on the CPU, to the steps that
get the CUDA runtime's environment, and `reads_corrections` names it as the step whose
fingerprint covers the owner's corrections.
`settings` returns the part of `JobSettings` a step reads, so a changed cut score reruns cue
building and nothing before it, and a changed output format reruns only the output. `revision`
is 1 for every step but those listed in `REVISIONS` (cue building is at 2: a cue still too
short shares a neighbour; the quality check is at 4: its findings name their utterance, and it
checks the words of each Fix It change the owner has not checked again, reading the re-decodes
for that), which makes outputs written by other code stale. `timeout` is 180 minutes for separation, 120 for
Whisper, adjudication and the sound cues, and 60 for the rest. `outputs` lists the files a
finished step leaves, the subtitle file beside the video, in the job's output format, among them.

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
  - a change to what a step writes raises its revision (the module header).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow and its inputs.
