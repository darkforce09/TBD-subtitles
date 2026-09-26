# Output stage

The output stage: the subtitle file installed beside the video with the video's base name, and any
different file it replaces backed up into the job's work directory first.

## Contents

```text
crates/stages/src/output/
├── mod.rs  `subtitle_path` and `install`: back up, write a part file, rename into place
└── tests/  unit tests for the file name, the backup and the refusal of a video named `.srt`
```

## How it works

`subtitle_path` is the video's path with the extension `.srt`. `install` compares what is already
there with the new text: an identical file is left alone and reported `unchanged`; a different one
is first copied into the job's backup folder as `<file name>.<stamp>`; then the text goes to
`<file>.srt.part` and is renamed over the file, so a player never reads half a file. A video whose
own name ends in `.srt` is refused, so the video is never overwritten.

## Boundaries

- Depends on: `std::fs` only.
- Used by: `crates/pipeline/src/tasks/layout.rs` (the output step, with the SRT text from
  `subtitle_formats::writers::srt`); `crates/pipeline/src/graph/mod.rs`,
  `crates/pipeline/src/runner/mod.rs` and `crates/pipeline/src/report/mod.rs`, which name the
  file through `subtitle_path`.
- Rules:
  - the stage runs inside the job runner, not in a worker, so it loads no model and starts no
    child process (`placement` in `crates/pipeline/src/graph/mod.rs`);
  - it is the only stage that writes beside the video rather than into the work directory
    (the crate header in `crates/stages/src/lib.rs`);
  - the file lands beside the video with its base name, a different file is backed up and an
    identical one left alone, and a video named `.srt` is refused
    (`the_file_lands_beside_the_video_with_its_base_name`,
    `a_different_file_is_backed_up_and_an_identical_one_is_left_alone`,
    `a_video_named_srt_is_refused` in `tests/output.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#11-output) — the file name, format and backup.
- [Decisions](/documentation/decisions/) — why subtitles live beside the video.
