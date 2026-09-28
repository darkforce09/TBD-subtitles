**Status:** live

# Decisions: the batch

The decisions about running many videos from the window at once: how Fix It runs on a whole
batch of finished videos and shares the `claude` calls it may make. The
[decision log](/documentation/decisions/) says how entries are written; the window's earlier
decisions, Fix It's among them, are in [desktop GUI](/documentation/decisions/desktop_gui.md).

### 2026-09-28 — Fix It runs on many videos at once under one cap on Claude calls

**Context:** Fix It ran on one video at a time: while it fixed one, every other video's Fix It
button was off ("Fix It is fixing another video; it fixes one at a time."), and a batch of Fix
Its could not be queued. The batch of Dressrosa 12–48 is 37 videos, and the owner wants Fix It on
100 and more at once, limited only by what the `claude` provider allows. Each run already has a
thread of its own and caps its own calls (`processes`); correction runs, which time Fix It's
changes, already run on four lanes of different videos at once.

**Decision:** Every video's Fix It runs on its own, at any time, however many others run: one
run per video, each on its own thread, never beside a run of its video. All runs' `claude` calls
pass one gate the window holds, capped by "Claude calls at once" (`fix_calls` in
`settings.toml`, 1–100, 32 by default), which an edit changes at once for the runs under way;
`processes` still caps each run alone. Calls wait in the order their runs started, so the videos
started first finish first; a call the provider answers as busy (rate limit, overloaded) is asked
again after 30, 60 and 120 s without holding a slot while it waits, and an answer kept from an
earlier run skips the gate. The four correction lanes time the changes of up to four videos at
once. A small Fix All button on the sidebar's Done heading starts Fix It on every finished video
with lines to fix that is not being fixed and whose subtitles are not being updated, oldest
finished first; the switch "Fix It after each job" (`fix_after_run`, off by default) starts Fix It
by itself on a video whose full run finished well, when it has lines to fix. While every call of
a run waits for a free slot, its note says "Waiting for a free Claude call." Rejected: a queue of
Fix Its run one after another, which would take a day for the batch.

**Consequences:** Stop ends only its own video's run, and a stopped or failed run names its video
in its toast. Each run that finishes still says so in its own toast, with See Changes. The window
redraws each second while a run goes, so the waiting note stays current. The tests hold it:
`apps/tbd_subtitles/src/application/tests/rendering_fix_many.rs` (two videos at once, Stop on
one, Fix All, Fix It after each job on and off, the waiting note, the cap following its setting)
and `crates/inference/src/llm/call_gate/tests/`.

**Supersedes:** the one-video-at-a-time rule and "A batch of Fix Its cannot be queued" of
2026-09-28 — Fix It: a stronger model fixes the flagged lines in three passes.
