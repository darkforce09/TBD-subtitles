# Check Text

Review and correct Japanese writing translated during the same video job as dialogue.

## Contents

```text
apps/tbd_subtitles/src/text_review/
├── mod.rs     feature exports
├── models/    text sessions, pictures and events
├── services/  background loading, corrections and playback
└── ui/        occurrence list, editor and comparison preview
```

## How it works

The application loads a finished job's visual rows and files on a background thread. Changes to wording, times and presentation are saved in one write transaction of the job's database, a change that rejects an occurrence's English first taking its signs out of the sign library (`library.redb`), then the existing correction queue regenerates the affected visual steps and combined ASS. Preview renders that ASS through FFmpeg. For a job that writes a localized video, the preview can instead play `<video>.localized.mkv` with its own subtitle file, show each occurrence's replaced plate before that video is written, lay the erase mask over the original picture, and say whether the occurrence was replaced in the video; the selected occurrence's plate and mask are decoded off the window thread.

## Public surface

- `models::Session` and `Event` carry the borrowed view and actions.
- `services` load, save and play; `ui` renders without changing application state.

## Boundaries

- Depends on: the core UI, the job's database and the sign library through `pipeline`, and media
  preview commands.
- Used by: the application composition layer.
- Rules: source video and dialogue corrections remain untouched; work never blocks rendering.

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — feature composition.
