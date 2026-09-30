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
└── tests/     unit tests for the merge, whole runs, a stop, a second run and a stale record
```

## How it works

```text
fix_video ─▶ JobStore ─▶ inputs::load ─▶ stages::fix_it::run (claude, through cache)
                                                   │
             review.json ◀─ update_corrections(merge) ◀─ fix.json
```

`fix_video` makes one `ClaudeCli` of the chosen model per worker, with the cancel token's flag,
so Stop kills the running `claude` processes, and wraps it in `inference::llm::call_gate::Gated`
on the run's seat (`FixOptions::calls`): each call takes a slot at the gate the runs share, runs
started earlier go first, a Stop ends a wait, and a busy answer is asked again after 30, 60 and
120 s. `fix_job` opens the job's store (`work_dir::JobStore::open`), sharing the handle of a
job of the same video running in this process and failing with the busy error kind when another
process owns the job, and refuses a job
whose quality check or output step has not finished, or whose corrections changed since its last
run ("your latest corrections are not in the subtitles yet"). It reads the sheet with the
re-decoded alternatives, the lines with the corrections in place, the corrections, `qc.json`,
`reviewed.json`, the main engine's words and the earlier runs' `fix.json` when it is current
(`FixRecord::is_current`: its `adjudication` is empty or matches the job's re-adjudication
fingerprint; a missing, unreadable or stale one counts as none). What that record answered
(`stages::fix_it::items::Answered`) is not asked again. It runs the passes with each model wrapped in
`cache::CachedModel`: a call whose answer is kept under `fix/calls/` is answered from disk at no
cost and without a slot at the gate, and every new answer is kept first. Progress comes out as a `FixStage` (reading, fixing a
family, checking, saving) with the calls done.

A stopped run returns a cancelled `PipelineError` and changes nothing; its answered calls stay, so
the next run asks only what is left. A finished run writes `fix.json`, then puts each kept or
accepted line of this run into `review.json` inside `work_dir::update_corrections` as a
`Chosen::FixIt` correction with the model and the reason, writes `fix.json` again with what was
applied, and removes `fix/calls/`. A line the owner settled, even one saved while Fix It ran,
keeps the owner's correction and is listed in `kept_yours`.

The written record is this run's lines and the earlier record's lines this run did not ask about,
sorted by id; its calls, cached calls, tokens, cost and failed calls add to the earlier ones.
`before` is the earlier record's, or `FixBefore::of` this run's `qc.json` when there is none, so
it keeps the problems before the first run. `adjudication` is the job's re-adjudication
fingerprint, empty when the job has none.

## Public surface

- `fix_video`, `FixOptions`, `FixStage`, `FixProgress` and `FixOutcome`: the window's Fix It and
  the `fix` subcommand.
- `fix_job`: the same run with any model maker, for tests.
- `merge` and `Merged`: the owner-first merge.

A call answered from `fix/calls/` makes no model call; it is logged as one `fix_it` line, "Answer
reused from an earlier run", with its purpose.

## Boundaries

- Depends on: `stages::fix_it`, `stages::adjudication::redecode`, `inference::llm`,
  `crate::work_dir` (its `JobStore` too), `crate::tasks::corrected_lines`, `job_model`,
  `sha2`.
- Used by: `apps/tbd_subtitles/src/application/` (the window's Fix It) and
  `apps/tbd_subtitles/src/cli/` (the `fix` subcommand).
- Rules:
  - an owner's correction is never replaced, even one saved during the run
    (`a_correction_the_owner_saves_during_the_run_wins` in `tests/fix_it.rs`);
  - a stopped run changes nothing and the next run reuses its answers
    (`a_stopped_run_changes_nothing_and_the_next_run_reuses_its_answers`);
  - a job that is not ready is refused (`a_job_that_is_not_ready_is_refused`);
  - a second run asks only what is left, keeps the first run's other lines and `before`, and adds
    up the cost (`a_second_run_asks_only_what_is_left_and_keeps_what_the_first_answered`);
  - a record from before the video was adjudicated again is ignored
    (`a_record_from_before_the_video_was_adjudicated_again_is_ignored`);
  - an answer kept on disk takes no slot at the gate (`a_kept_answer_takes_no_claude_slot`).

## Related documentation

- [Fix It](/documentation/features/fix_it.md) — the feature as the owner uses it.
- [Pipeline](/documentation/architecture/pipeline.md#fix-it) — Fix It after a finished job.
