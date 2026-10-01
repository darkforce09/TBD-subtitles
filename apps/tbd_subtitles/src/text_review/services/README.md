# Text review services

Background file operations and bounded original/rendered preview playback.

## Contents

```text
apps/tbd_subtitles/src/text_review/services/
├── tests/        correction, retry, artifact and replacement regression tests
├── localized.rs  the localized video's rows and files, replacements, keyframe plates and pictures
├── mod.rs        service exports
├── player.rs     cancellable FFmpeg comparison playback
└── session.rs    the job's rows read in one snapshot; corrections changed in one transaction
```

## How it works

The application calls these services on background threads. Preview reads two bounded frame streams and supplies the latest comparison: the source, and on the right the source with the exported ASS burned in or, in localized mode, `<video>.localized.mkv` with `<video>.localized.ass` burned in at the same time and pace; while the localized video is not written, only the source streams. Dropping a player stops its child processes.

`session::load` reads, in one snapshot of the job's database (`pipeline::work_dir::read_stored`, which shares the window's own handle while the job runs in this process), the typeset text (`outputs/text_typeset`, else `outputs/text_review` before typesetting ran; a broken typeset text is an error, never the older review), the job record, the probe, the output record and the owner's text corrections (`corrections/text`); `session::save` changes those corrections in one write transaction of the job's database (`update_text_corrections`), which rereads them inside it, so a change committed meanwhile is kept. `localized::rows` takes the localized video's record (`outputs/localized_video`) the replacements the read-back check left (`outputs/text_verify`) and the telling reading of each occurrence it checked (its `readings` rows: the first failure, else the weakest match) from the same snapshot, and `localized::load` joins them with the localized subtitle file the output record names for a job whose settings write a localized video; a missing or broken record is absent, never an error. It finds each occurrence's keyframe plate as composition chose it (the keyframe time's share of the span, then the plate covering that frame or the nearest), decodes the selected occurrence's replaced plate and erase mask at most 720 pixels on a side, words the status ("Replaced in the video", "Not replaced in the video: …") and what the read-back check read ("Checked: English reads back as …", "Checked: Japanese still reads …") and maps a plate's rectangle to shares of the picture.

## Boundaries

- Depends on: the job's database through `pipeline::work_dir`, media commands, child processes
  and text review models.
- Used by: application text actions.
- Rules: source files are read-only; a correction change is one write transaction and keeps
  concurrent changes (`undo_rereads_current_corrections_and_preserves_other_occurrences`); a
  reader shares the handle of a job running in this process
  (`a_reader_shares_the_handle_of_a_job_running_in_this_process` in `tests/session.rs`).

## Related documentation

- [Check Text](/apps/tbd_subtitles/src/text_review/) — the feature.
