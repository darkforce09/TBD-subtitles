# Held frames

The decoded frames detection keeps for a while: the samples and the frames between them until
their group is observed and bisected, and the samples that may still become a keyframe.

## Contents

```text
crates/stages/src/onscreen_text/detect/window/
├── candidates.rs  keyframe candidates per occurrence, shared frames and the 4 GiB budget
├── mod.rs         `HeldSample`, `Group` and `Gathering`: samples with their gaps, grouped by job
└── tests/         grouping bounds, frame lookup, candidate windows, sharing and the budget
```

## How it works

`Gathering` collects decoded frames: a frame between samples joins the gap, and a sample takes the
gap with it as a `HeldSample`, with its padded picture when it is screened. The group closes once
its pictures fill a screening batch or it holds twice a batch of samples; the gap after its last
sample stays for the next group, so every frame belongs to exactly one group and a transition seen
at a sample is bisected inside that sample's own gap. The samples are shared (`Arc`), so the
candidates can keep one after its group is gone.

`Candidates` keeps one window of `(index, time)` entries per occurrence and every held frame once,
with how many windows hold it. Offering adds a sample; pruning drops the samples before the last
one at or before the middle of the occurrence's start and its latest sample, which can no longer
be nearest its final middle; choosing keeps only the keyframe, or marks the occurrence for a still
from the video when its window no longer holds it. Past the budget, the active window holding the
most samples falls back and releases its frames; kept keyframes stay, since choosing and pruning
only release frames. `into_frames` hands confirmation the held keyframes by index.

## Boundaries

- Depends on: `media_io::yuv::YuvFrame` and `inference::ocr::pool::PaddedFrame`.
- Used by: the scan coordinator and its tracker, the probes (frame lookup) and confirmation (held
  keyframes).
- Rules: a gap holds at most `k - 1` frames and a closed group at most twice a batch of samples
  (`tests/window.rs`); a frame is counted once however many windows hold it, and the held bytes
  never stay above the budget (`tests/candidates.rs`).
