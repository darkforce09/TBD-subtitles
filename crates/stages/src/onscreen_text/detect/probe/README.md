# Bisection probes

Where exactly a region enters or leaves between two samples: the frames between them are screened
in a few lockstep steps, ahead of the screening batches that wait.

## Contents

```text
crates/stages/src/onscreen_text/detect/probe/
├── mod.rs     `Transition`, `narrow`: probe pictures from held frames, their screens, presence
├── search.rs  `Search`, `Seek` and `bisect`: lockstep interval searches, answers side by side
└── tests/     bisection bounds, lockstep steps, exact entries and exits, and probes outside a group
```

## How it works

Each `Transition` carries the occurrence, the group position of the sample whose gap holds the
change, the search interval, the region's box, its fixed anchor box and the anchor's signature.
`narrow` runs `bisect` over all of a group's transitions: every step takes the distinct probe
frames not yet screened, converts them from the group's held frames to padded pictures side by
side, and hands them to the caller's screen, which submits them as probe jobs and waits for their
answers. Each open search's probe is then answered in parallel: present when a screened box
overlaps the region by more than 0.45 and the frame's luma at the anchor box still matches the
anchor. The answers are applied in search order, so the result never depends on thread timing.

## Boundaries

- Depends on: the sibling `window` (held frames), `crops` (`picture_at`), `regions`, `screen`
  (`padded`) and `timing` modules, and `rayon`.
- Used by: the scan coordinator, once per group.
- Rules: a search needs at most ceil(log2(k)) probes and each step screens its distinct probes in
  one call (`tests/search.rs`); a probe outside the group's held frames is an error, never a new
  decode (`tests/probe.rs`).
