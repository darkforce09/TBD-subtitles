# Text review services

Background file operations and bounded original/rendered preview playback.

## Contents

```text
apps/tbd_subtitles/src/text_review/services/
├── tests/        correction, retry, artifact and replacement regression tests
├── localized.rs  the localized video's files, replacements, keyframe plates and pictures
├── mod.rs        service exports
├── player.rs     cancellable FFmpeg comparison playback
└── session.rs    artifact loading and locked correction writes
```

## How it works

The application calls these services on background threads. Preview reads two bounded frame streams and supplies the latest comparison: the source, and on the right the source with the exported ASS burned in or, in localized mode, `<video>.localized.mkv` with `<video>.localized.ass` burned in at the same time and pace; while the localized video is not written, only the source streams. Dropping a player stops its child processes.

`localized` reads `visual/localized_video.json`, `visual/text_compose.json` and the localized subtitle file named in `output.json` for a job whose settings write a localized video; a missing or broken record is absent, never an error. It finds each occurrence's keyframe plate as composition chose it (the keyframe time's share of the span, then the plate covering that frame or the nearest), decodes the selected occurrence's replaced plate and erase mask at most 720 pixels on a side, words the status ("Replaced in the video", "Not replaced in the video: …") and maps a plate's rectangle to shares of the picture.

## Boundaries

- Depends on: pipeline files, media commands, child processes and text review models.
- Used by: application text actions.
- Rules: source files are read-only; correction writes are atomic and preserve concurrent edits.

## Related documentation

- [Check Text](/apps/tbd_subtitles/src/text_review/) — the feature.
