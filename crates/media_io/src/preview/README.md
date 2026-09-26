# Clip preview

The FFmpeg command lines that play a short clip for review: its sound through FFmpeg's `pulse`
output device, and its picture as raw RGBA frames of a given size and rate on stdout.

## Contents

```text
crates/media_io/src/preview/
├── mod.rs  `Clip`, `track_sound`, `stem_sound`, `frame_size` and `frames`
└── tests/  unit tests for the padding, the sound and frame arguments, and the frame size
```

## How it works

`Clip::around` pads a line's span on both sides, never before the video's start. `track_sound`
seeks the video and plays one audio track to `pulse` under the stream name "TBD Subtitles clip";
`stem_sound` reads a raw 16 kHz mono `f32` stem the same way. `pulse` drops what the sound server
still holds when FFmpeg exits (about two seconds by default), so both sound commands keep a
200 ms server buffer (`-buffer_duration`) and append one second of silence (`apad`) after the
clip: FFmpeg exits once only silence is left to play, and the clip's own span (`-ss`, `-t` on the
input) stays exact. `frame_size` keeps the picture's shape on even sizes, and `frames` scales and
resamples the picture to raw RGBA on stdout.

## Boundaries

- Depends on: `std` only; the commands are run by the caller.
- Used by: `apps/tbd_subtitles/src/line_review/services/clip_player.rs`.
- Rules: the video is only read (the module header); a clip never starts before the video
  (`a_clip_is_padded_but_never_before_the_video` in `tests/preview.rs`); the sound ends with a
  silence pad longer than the server buffer (a compile-time assertion in `mod.rs`, and
  `the_track_sound_goes_to_the_pulse_device` and `a_stem_is_read_as_raw_16_khz_mono` in
  `tests/preview.rs`).
