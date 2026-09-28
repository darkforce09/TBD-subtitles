# Fix It runner

[Fix It](/documentation/glossary.md#fix-it) on a finished job: the job's outputs read, the three
passes of `stages::fix_it` run with the `claude` CLI, and the changes they kept written into the
job's corrections, where a correction run then times them.

## Contents

```text
crates/pipeline/src/fix_it/
├── cache.rs   each answered call kept in `fix/calls/`, so a stopped run resumes at no cost
├── inputs.rs  whether the job is ready, and everything Fix It reads from its work directory
├── merge.rs   the kept changes into the corrections, the owner's own corrections first
├── mod.rs     `fix_video`, `fix_job`, `FixOptions`, `FixStage`, `FixProgress` and `FixOutcome`
└── tests/     unit tests for the merge, a run end to end, an owner's save meanwhile, a stop
```

## How it works

```text
fix_video ─▶ job lock ─▶ inputs::load ─▶ stages::fix_it::run (claude, through cache)
                                                   │
             review.json ◀─ update_corrections(merge) ◀─ fix.json
```

`fix_video` makes one `ClaudeCli` of the chosen model per worker, with the cancel token's flag,
so Stop kills the running `claude` processes. `fix_job` takes the job lock and refuses a job
whose quality check or output step has not finished, or whose corrections changed since its last
run ("your latest corrections are not in the subtitles yet"). It reads the sheet with the
re-decoded alternatives, the lines with the corrections in place, the corrections, `qc.json`,
`reviewed.json` and the main engine's words, and runs the passes with each model wrapped in
`cache::CachedModel`: a call whose answer is kept under `fix/calls/` is answered from disk at no
cost, and every new answer is kept first. Progress comes out as a `FixStage` (reading, fixing a
family, checking, saving) with the calls done.

A stopped run returns a cancelled `PipelineError` and changes nothing; its answered calls stay, so
the next run asks only what is left. A finished run writes `fix.json`, then puts each kept or
accepted line into `review.json` inside `work_dir::update_corrections` as a `Chosen::FixIt`
correction with the model and the reason, writes `fix.json` again with what was applied, and
removes `fix/calls/`. A line the owner settled, even one saved while Fix It ran, keeps the
owner's correction and is listed in `kept_yours`.

## Public surface

- `fix_video`, `FixOptions`, `FixStage`, `FixProgress` and `FixOutcome`: the window's Fix It and
  the `fix` subcommand.
- `fix_job`: the same run with any model maker, for tests.
- `merge` and `Merged`: the owner-first merge.

## Boundaries

- Depends on: `stages::fix_it`, `stages::adjudication::redecode`, `inference::llm`,
  `crate::work_dir`, `crate::resume` (the job lock), `crate::tasks::corrected_lines`, `job_model`,
  `sha2`.
- Used by: `apps/tbd_subtitles/src/application/` (the window's Fix It) and
  `apps/tbd_subtitles/src/cli/` (the `fix` subcommand).
- Rules:
  - an owner's correction is never replaced, even one saved during the run
    (`a_correction_the_owner_saves_during_the_run_wins` in `tests/fix_it.rs`);
  - a stopped run changes nothing and the next run reuses its answers
    (`a_stopped_run_changes_nothing_and_the_next_run_reuses_its_answers`);
  - a job that is not ready is refused (`a_job_that_is_not_ready_is_refused`).

## Related documentation

- [Fix It](/documentation/features/fix_it.md) — the feature as the owner uses it.
- [Pipeline](/documentation/architecture/pipeline.md#fix-it) — Fix It after a finished job.
