# Shot-change scan

FFmpeg's `scdet` filter over a small scaled copy of the video: the times of the
[shot changes](/documentation/glossary.md#shot-change) that cue timing snaps to. The module's code
is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/media_io/src/shot_changes/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/media_io/src/lib.rs` declares it as a public module.
- Rules: none of its own beyond the crate's; see the
  [crate README](/crates/media_io/README.md#boundaries).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#8-shot-changes) — the `scdet` command
  and its measured speed.
- [Pipeline](/documentation/architecture/pipeline.md#9-cue-building) — how cues snap to shot
  changes.
