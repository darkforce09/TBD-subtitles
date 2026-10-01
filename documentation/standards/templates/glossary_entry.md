**Status:** live

# Template: glossary entry

**When to use:** a term the documents use with a project-specific meaning, or an abbreviation they
use, defined once in the [glossary](/documentation/glossary.md) at the documentation root. Every
document links the first use of such a term to its entry, and every entry that names code is
checked against it. The [documentation standards](/documentation/standards/documentation_standards.md)
hold the terminology rule every document follows.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. The heading is
the term as prose writes it, with a capital first letter and abbreviations in capitals (`### Cue`,
`### Forced alignment`, `### SDH`), so its anchor stays stable: the entry `### Worker process` is
linked as `#worker-process`. Entries run in alphabetical order. The In code and See lines are
added when they have something to hold, and left out otherwise.

````markdown
### <Term>

<Definition: one to three sentences saying what the term means in this project, and what it is
not when another term sits close to it.>

In code: <the types, functions, subcommands and files that carry the concept, in backticks with
their repository paths; where the code spells the concept another way, say so>

See: <links to the related entries and to the documents that go deeper>
````

A document links the term's first use to the entry, with the glossary's repository-root path and
the anchor:

```text
[worker process](/documentation/glossary.md#worker-process)
```

## Worked sample

Written from the glossary's entry for worker process, with In code and See lines added from the
step graph in `crates/pipeline/src/graph/mod.rs`, the app's command line in
`apps/tbd_subtitles/src/cli/`, the worker binaries and the decision log. The sample sits in a
fenced block, so no gate reads its links or paths.

````markdown
### Worker process

A `worker <step>` subcommand of an app binary that runs one step in its own process and exits
when done, freeing VRAM. The main binary `tbd-subtitles` hosts the ONNX Runtime, FFmpeg and
`claude` workers; `tbd-subtitles-ggml` hosts Whisper, because ggml and ONNX Runtime cannot share
a process; `tbd-subtitles-llm` hosts the local translation model through mistral.rs. A worker
takes its inputs and returns its outputs over the [worker channel](#worker-channel) and never
opens the job's database.

In code: `pipeline::graph::placement` in `crates/pipeline/src/graph/mod.rs`, which answers
`Placement::Worker(Binary)` for a step that runs in a worker; `pipeline::tasks::worker_main`, which
every binary's `worker` entry calls; the `worker <STEP> <JOB_DIR>` subcommand in
`apps/tbd_subtitles/src/cli/worker_command.rs`, and the entries in
`apps/tbd_subtitles_ggml/src/main.rs` and `apps/tbd_subtitles_llm/src/main.rs`.

See: [Step](#step), [Worker channel](#worker-channel),
[Each GPU stage runs in its own worker process](/documentation/decisions/foundations.md#2026-09-25--each-gpu-stage-runs-in-its-own-worker-process).
````
