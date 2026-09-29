# Visual validation

Repeatable recognition and acceptance checks using the production visual backends.

## Contents

```text
tools/visual_validation/
├── Cargo.toml  validation tool dependencies
├── pilots/     independent frame annotations and explicit clipped-text limitations
└── src/        commands and annotated evaluation
```

## How it works

The tool downloads checksum-pinned models, recognizes owner-provided stills, extracts bounded pilot clips and compares production visual documents against independent annotations. Coverage, timing and tracking errors are measured separately from model confidence.

## Getting started

Run `cargo run -p visual_validation -- --help`. Run model inference and FFmpeg commands on the host with its CUDA driver and current FFmpeg.

## Configuration

Annotations contain source frame rate, height and expected occurrences with text, start/end times and frame quadrilaterals. Media paths are local and source files are never modified.

## Public surface

- `fetch` downloads the OCR models.
- `image` recognizes a supplied PNG and writes crop and observation artifacts.
- `clip` extracts a pilot of up to two minutes.
- `run` executes all six visual steps with production workers, resume fingerprints and measured
  time/RAM/VRAM. It accepts optional dialogue cues, uses the One Piece glossary and writes a preview
  ASS in a dedicated work directory. Source videos and installed subtitles remain untouched.
- `inspect` prints compact occurrence readings, translations, confidence and review flags.
- `scenarios` creates an eight-second Japanese source fixture and independent annotations for
  scrolling credits, fading lyrics, vertical writing, a brief sign, a cut and repeated writing.
- `evaluate` writes a machine-readable verdict and exits unsuccessfully when acceptance fails.
- `font-candidate` inspects an official Google Fonts candidate before checksum pinning.

## Boundaries

- Depends on: the production model, stage, media, pipeline and subtitle crates.
- Used by: development validation on owner-provided media.
- Rules: no models are converted, missing readable occurrences fail, and unsafe tracks need an explicit fallback.

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md) — host execution.
