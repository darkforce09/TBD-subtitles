# Sound events stage

Sound events on both stems: a tagger scores sliding windows, each class's scores are smoothed, and
stretches over a class's threshold become events; the events, Whisper's sound tags and the songs
are then cut down to the candidate [sound cues](/documentation/glossary.md#sound-cue) the language
model chooses from.

## Contents

```text
crates/stages/src/sound_events/
├── candidates.rs  songs from lyric runs, effects, voices away from speech, Whisper's tags
├── classes.rs     the subtitle classes per stem, with thresholds and minimum lengths, by AudioSet name
├── mod.rs         the `Tagger` trait, `score_stem`, `events` and `mean_score`
└── tests/         unit tests for event cutting, smoothing, class names, the mean, candidates
```

## How it works

`score_stem` reads 2 s windows every 0.5 s from a 16 kHz stem and scores them in batches of 64.
`events` smooths one class's scores with a three-window median and cuts stretches that stay over
the class's threshold for its minimum length; an event runs from half a hop before its first
window's centre to half a hop after its last. `classes.rs` lists effects and music for the
background stem and non-speech voices and singing for the vocal stem, looked up in the rated
AudioSet table so a misspelt class fails the tests.

`candidates::candidates` runs in the sound-cue step, on the settled lines. Runs of `LYRIC` lines
become one song each, joined across instrumental breaks under 30 s with no spoken line between;
music and singing events, and anything inside a song, are left out, so music reaches the cues
only as songs. A voice event on the vocal stem is kept only
when less than half of it overlaps the backbone's speech, and groans, sighs and grunts must also
pass a higher peak and length. Whisper's `*tags*`, `[tags]` and `(tags)` become candidates too.
Same-class candidates closer than 1 s merge, and the survivors are numbered `S001`, `S002`, … in
time order.

## Boundaries

- Depends on: `media_io::pcm_stream::read_f32_range`, `soundevents-dataset` (the label table),
  `inference::onnx::ced` (the `Tagger` implementation), `job_model::outputs` (`SoundEvent`,
  `SoundCandidate`, `Utterance`, `Line`, `EngineTranscript`, `TimeSpan`).
- Used by: `crates/pipeline/src/tasks/sounds.rs` (the sound-events step, and the candidates for
  the sound-cue step) and `tools/stack_spike/` (the sound-event item).
- Rules:
  - a one-window spike is smoothed away (`a_one_window_spike_is_smoothed_away`); every class named
    exists in the rated set (`every_subtitle_class_exists_in_the_rated_set` in
    `tests/sound_events.rs`);
  - each lyric run is one song, and music and events inside songs are not candidates
    (`lyric_runs_become_one_song_each`, `music_and_events_inside_songs_are_not_candidates` in
    `tests/candidates.rs`);
  - voices during speech and weak groans are dropped
    (`voices_during_speech_and_weak_groans_are_dropped`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#8-sound-events) — classes per stem and how
  candidates reach adjudication.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#music) — why music
  becomes a cue only for a song.
