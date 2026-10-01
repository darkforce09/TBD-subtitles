# PNG writer

One thread that writes detection's crops and keyframe images while confirmation goes on.

## Contents

```text
crates/stages/src/onscreen_text/detect/writer/
├── mod.rs  `PngThread` and `PngJob`: a bounded channel to one writer thread, keyframes shrunk to 1280 wide
└── tests/  sizes written, errors that come back, an empty run
```

## Boundaries

- Depends on: the sibling `png` module of `onscreen_text` (synced writes) and `image`.
- Used by: confirmation.
- Rules: every image is written whole and synced before `finish` returns; the first write error
  comes back from the next `send` or from `finish` (`tests/writer.rs`); at most eight images wait.
