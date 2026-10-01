# Output stage

The output stage: the subtitle file installed beside the video with the video's base name, in the
job's output format, any different file it replaces backed up into the job's work directory
first, and the job's file of an earlier format moved there too; with a localized video, the
localized video's own subtitle file installed beside it the same way.

## Contents

```text
crates/stages/src/output/
├── mod.rs  the subtitle and localized file names, `install` and `install_localized_subtitles`
└── tests/  unit tests for the file name, the backup, a format change and the refusals
```

## How it works

`subtitle_path` is the video's path with the format's extension (`.srt`, `.vtt` or `.ass`).
`install` compares what is already at the new path with the new text: an
identical file is left alone and reported `unchanged`; a different one is first copied into the
backup folder as `<file name>.<stamp>`; then the text goes to `<file>.part` and is renamed over the
file, so a player never reads half a file. After installation succeeds, the video's file of the
earlier format is moved into the backup folder. A failed ASS write therefore leaves the existing
SRT available to VLC. A video whose own name is its subtitle file name is
refused, so the video is never overwritten.

`localized_video_path` and `localized_subtitle_path` name the localized video's files,
`<base name>.localized.mkv` and `<base name>.localized.ass`. `install_localized_subtitles` installs
the second through the same backup and part file, and retires nothing.

## Boundaries

- Depends on: `std::fs` and `job_model::job::OutputFormat`.
- Used by: `crates/pipeline/src/tasks/layout.rs` (the output step, with the text from the
  format's writer in `subtitle_formats::writers`, and the localized subtitle file);
  `crates/pipeline/src/tasks/localized.rs` (`localized_video_path`, for the localized-video
  step); `crates/pipeline/src/runner/mod.rs`, `crates/pipeline/src/report/mod.rs` and
  `apps/tbd_subtitles/src/job_report/services/report_loading.rs`, which name the file through
  `subtitle_path`.
- Rules:
  - the stage runs inside the job runner, not in a worker, so it loads no model and starts no
    child process (`placement` in `crates/pipeline/src/graph/mod.rs`);
  - it is the only stage that writes subtitle files beside the video rather than into the work
    directory, and `localize` writes only the video at the path its task hands it (the crate
    header in `crates/stages/src/lib.rs`);
  - the file lands beside the video with its base name, a different file is backed up and an
    identical one left alone, and a video named like its subtitle file is refused
    (`the_file_lands_beside_the_video_with_its_base_name`,
    `a_different_file_is_backed_up_and_an_identical_one_is_left_alone`,
    `a_video_named_like_its_subtitles_is_refused` in `tests/output.rs`);
  - a format change moves the job's old file aside only once the new file is in place, and only
    the video's own subtitle files are ever moved (`a_new_format_moves_the_jobs_old_file_aside`,
    `a_failed_ass_install_keeps_the_video_and_old_srt_in_place_until_retry_succeeds`,
    `only_the_videos_own_subtitle_files_are_moved_aside`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#11-output) — the file name, format and backup.
- [Decisions](/documentation/decisions/) — why subtitles live beside the video.
