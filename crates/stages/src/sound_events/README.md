# Sound events stage

Sound events on both stems: a tagger scores sliding windows, each class's scores are smoothed,
and stretches over a class's threshold become candidate [sound cues](/documentation/glossary.md#sound-cue)
and the music and singing stretches.

## Contents

```text
crates/stages/src/sound_events/
├── classes.rs  the subtitle classes per stem, with thresholds and minimum lengths, by AudioSet name
├── mod.rs      the `Tagger` trait, `score_stem`, `events` and `mean_score`
└── tests/      unit tests for event cutting, smoothing, class names and the windowed mean
```

## How it works

`score_stem` reads 2 s windows every 0.5 s from a 16 kHz stem and scores them in batches of 64.
`events` smooths one class's scores with a three-window median and cuts stretches that stay over
the class's threshold for its minimum length; an event runs from half a hop before its first
window's centre to half a hop after its last. `classes.rs` lists effects and music for the
background stem and non-speech voices and singing for the vocal stem, looked up in the rated
AudioSet table so a misspelt class fails the tests.

## Boundaries

- Depends on: `media_io::pcm_stream::read_f32_range`, `soundevents-dataset` (the label table),
  `inference::onnx::ced` (the `Tagger` implementation), `job_model::outputs::SoundEvent`.
- Used by: `tools/stack_spike/` (the sound-event item).
- Rules: a one-window spike is smoothed away (`a_one_window_spike_is_smoothed_away`); every class
  named exists in the rated set (`every_subtitle_class_exists_in_the_rated_set`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#8-sound-events) — classes per stem and how
  candidates reach adjudication.
