# Text review services

Background file operations and bounded original/rendered preview playback.

## Contents

```text
apps/tbd_subtitles/src/text_review/services/
├── tests/      correction, retry and artifact regression tests
├── mod.rs      service exports
├── player.rs   cancellable FFmpeg comparison playback
└── session.rs  artifact loading and locked correction writes
```

## How it works

The application calls these services on background threads. Preview reads two bounded frame streams and supplies the latest comparison. Dropping a player stops its child processes.

## Boundaries

- Depends on: pipeline files, media commands, child processes and text review models.
- Used by: application text actions.
- Rules: source files are read-only; correction writes are atomic and preserve concurrent edits.

## Related documentation

- [Check Text](/apps/tbd_subtitles/src/text_review/) — the feature.
