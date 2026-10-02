# Scan coordinator

The heart of detection: it reads every decoded frame, screens the samples ahead in batches while
decoding goes on, and applies the results strictly in sample order.

## Contents

```text
crates/stages/src/onscreen_text/detect/scan/
├── flight.rs   screening jobs in flight: numbered on submission, answered in any order, kept by number
├── mod.rs      `run`: the decode loop, groups waiting for their jobs, probes, then confirmation
├── tests/      in-order application, one against two sessions, probe priority, the candidates' budget
└── tracker.rs  region following in sample order, entries and exits, keyframes and the noise filter
```

## How it works

`run` checks each frame against the timeline and the video's size, keeps the sample choice and the
luma repeat check, converts the samples it screens and gathers them into groups. When a group
closes, `Flight` submits its pictures as one `Priority::Screen` job and the group waits in a queue
of at most six groups per session; while it is full, the coordinator waits for the front group's
result, keeping any other result that arrives first. Whenever the front group's job is answered
(or it has none, every sample repeating), `Tracker::observe` follows its samples' regions in order,
recording the entries and exits of writing that persists and letting go of the keyframe candidates
of writing that ends sooner. `Tracker::forget_entries_outside` then keeps every pending entry
whose first sample the next group carries; the last `max(MIN_BISECTION_SAMPLES − 1, 1)` samples
observed (two) are carried with their gaps, as many as writing can be seen before it persists.
`probe::narrow` bisects the transitions found, over the carried samples and the group's own, its
probe pictures submitted as `Priority::Probe` jobs and waited for by number, and
`Tracker::settle` applies the entry and exit frames, chooses keyframes and prunes the active
occurrences' candidates. After the last frame the remaining groups drain in order, the tracker
closes and confirmation runs on the same sessions.

## Boundaries

- Depends on: the sibling `screen`, `window`, `probe`, `regions`, `crops`, `confirm`, `source` and
  `timing` modules, `inference::ocr::pool`, `media_io::yuv` and `rayon`.
- Used by: `detect::scan_measured`.
- Rules: a group is observed only after every group before it and only once its job is answered,
  and its bisection ends before the next group is observed; every job number is distinct and every
  result is checked for one region list per frame (`tests/flight.rs`); the document does not
  depend on how many sessions run or the order they answer in (`tests/scan.rs`).
