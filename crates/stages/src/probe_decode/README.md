# Probe and decode stage

The probe and decode stage: ffprobe the video, choose its audio track, and stream that track as
16 kHz mono `f32` into the work directory, where the detector and the engines read it. The
[shot-change](/documentation/glossary.md#shot-change) scan is a step of this stage that the
pipeline runs straight through `media_io`.

## Contents

```text
crates/stages/src/probe_decode/
├── mod.rs  `pick_track` and `probe_and_decode`: probe, choose the track, stream the mix to a file
└── tests/  unit tests for the track choice
```

## How it works

`probe_and_decode` probes the video with `media_io::probe`, picks the track with `pick_track` (the
one at the given `-map 0:a:<n>` position, else the English one), and streams it through FFmpeg in
one-second chunks into the mix file, which `media_io` writes as a part file and renames once
FFmpeg has finished cleanly; after each chunk its progress callback hears the seconds decoded of
the probed length. The
returned `Decoded` holds the probe result, the chosen track and the samples written; the pipeline
keeps it as `probe.json` (`job_model::outputs::ProbeDecoded`).

## Boundaries

- Depends on: `media_io` (`probe`, `pcm_stream::PcmStream` and `F32FileWriter`, `MediaError`,
  `Programs`), `job_model::outputs::{ProbeResult, AudioStream}`.
- Used by: `crates/pipeline/src/tasks/media.rs` (the probe-and-decode step).
- Rules:
  - the step runs in a worker process of the main binary, which measures FFmpeg's memory as its
    child's (`placement` in `crates/pipeline/src/graph/mod.rs`);
  - a chosen track wins over the language tag, and several untagged tracks need a choice
    (`a_chosen_track_wins_over_the_language_tag`, `several_untagged_tracks_need_a_choice` in
    `tests/probe_decode.rs`);
  - the video is only read, and the mix is never held in memory whole (the header in `mod.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — the probe, the audio
  stream and the shot-change scan.
