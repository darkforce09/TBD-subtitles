# Video probe

ffprobe's JSON for a video, read into the probe result (duration, the video stream and its frame
rate, the audio tracks), and the choice of the English audio track.

## Contents

```text
crates/media_io/src/probe/
├── mod.rs  `probe` runs ffprobe, `parse` reads its JSON, `english_track` picks the track to decode
└── tests/  unit tests for parsing, colour tags, bit rates, `und`, and the English and ambiguous tracks
```

## Boundaries

- Depends on: `child_process::Run` for ffprobe (60 s deadline); `serde_json`; the
  `job_model::outputs` probe types.
- Used by: the probe-and-decode stage in `crates/stages/src/probe_decode/`,
  `tools/stack_spike/` (the decode and separate items) and `tools/visual_validation/src/pilot.rs`.
- Rules:
  - `und` is no language, a track tagged `eng` or `en` wins, a single track is taken, and several
    untagged tracks are refused rather than guessed
    (`several_untagged_tracks_are_ambiguous` and its neighbours in `tests/probe.rs`);
  - the video file is only read (the crate header in `crates/media_io/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — the probe fields and
  how the English track is picked.
