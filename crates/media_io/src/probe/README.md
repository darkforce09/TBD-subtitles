# Video probe

ffprobe's JSON for a video: its streams, languages, durations, frame rate and start time, and the
choice of the English audio track. The module's code is not written yet; `mod.rs` holds only its
header.

## Contents

```text
crates/media_io/src/probe/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/media_io/src/lib.rs` declares it as a public module.
- Rules: the video file is only read (the crate header in `crates/media_io/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — the probe fields and
  how the English track is picked.
