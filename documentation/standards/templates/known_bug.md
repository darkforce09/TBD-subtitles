**Status:** live

# Template: known bug

**When to use:** a defect that is recorded rather than fixed now, because it is minor, waits behind
other work or comes from the environment; and a fixed defect whose analysis is worth keeping. Each
entry is one file in a known bugs folder under the documentation root, named
`kb_<NNN>_<subject>.md` with the next free number and headed `KB-<NNN>`. The folder is created with
its first entry, together with its README from the
[documentation folder template](/documentation/standards/templates/readme_documentation_folder.md),
which lists every entry with its status. A resolved entry stays in the folder with its status set
to resolved. The [README standard](/documentation/standards/readme_standard.md) holds the writing
rules an entry shares with other documents.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. The sections
come in this order, spelled this way.

````markdown
**Status:** live

# KB-<NNN> — <the defect, as the user or developer meets it>

## Status

<Open, deferred or resolved, and why; the severity and the area. A resolved entry names what
fixed it.>

## Symptom

<What is seen, with error text and output quoted exactly, and the conditions that bring it on.>

## Cause

<Why it happens, as the code or the environment shows it, with the evidence that proves it and
what was ruled out.>

## Workaround

<What to do until it is fixed, or "None." when nothing is needed.>

## Fix

<The fix that shipped, naming the code it changed; or the fix a future change would make.>

## Related

- [<roadmap milestone, decision entry or document>](/documentation/<path to the document>) —
  <what it has to do with the bug>
````

There are no tickets: Related links the roadmap milestone whose checklist holds the fix, the
decision entries the bug touches, and the documents that cite it. Deferring a fix is the owner's
call; an entry says who deferred it and never defers work on its own.

## Worked sample

No known bug is recorded yet. The sample is written from the host and container facts in
`documentation/runbooks/development_environment.md`, checked by running `ffmpeg -version` inside
the `claude-desktop` container and through `distrobox-host-exec`, and from the `which` lookup in
`crates/child_process/src/lookup.rs`. The sample sits in a fenced block, so no gate reads its
links.

````markdown
**Status:** live

# KB-001 — FFmpeg run from the Claude Code container is 5.1, not the host's 8.1

## Status

Open, and out of the project's hands: the container is Debian 12, whose FFmpeg package is 5.1.
Severity medium for development only; the finished app runs on the host. Area: the development
environment and any FFmpeg call made while working inside the container.

## Symptom

Inside the `claude-desktop` container, `ffmpeg -hide_banner -version` prints

```text
ffmpeg version 5.1.9-0+deb12u1 Copyright (c) 2000-2026 the FFmpeg developers
```

while the same command through `distrobox-host-exec` prints `ffmpeg version 8.1.2`. Nothing fails
loudly: a probe, a decode or a timing measurement made inside the container silently uses the
older FFmpeg.

## Cause

The container has its own `/usr/bin/ffmpeg` from Debian 12. The host's FFmpeg is visible inside
it only as `/run/host/usr/bin/ffmpeg`, which no `PATH` entry names, so the container's copy
answers. `child_process::which`, the lookup the app's crates are built to find programs with,
takes the first match on the `PATH`, so an app run inside the container would call FFmpeg 5.1 too.

## Workaround

Run FFmpeg and ffprobe on the host: prefix each command with `distrobox-host-exec` inside the
container, and launch the app itself on the host.

## Fix

None planned in the code: the app runs on the host, where `/usr/bin/ffmpeg` is 8.1. A version
check at start-up that refuses an FFmpeg older than 8 would turn the silent difference into an
error.

## Related

- [Development environment](/documentation/runbooks/development_environment.md) — the host and
  container split, and the FFmpeg check.
- [Rust only; FFmpeg as the one external program](/documentation/decisions.md#2026-09-25--rust-only-ffmpeg-as-the-one-external-program)
  — why the app depends on the FFmpeg it finds.
- [M0.5 — Stack spike on Dressrosa 08](/documentation/roadmap.md#m05--stack-spike-on-dressrosa-08)
  — the FFmpeg streaming measurements, which must be taken on the host.
````
