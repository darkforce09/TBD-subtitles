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

The tool downloads checksum-pinned models, recognizes owner-provided stills, extracts bounded pilot clips and compares production visual documents against independent annotations. Coverage, timing and tracking errors are measured separately from model confidence. An annotated frame sample counts as observed when a document frame starts within one source frame of it or holds its geometry across it, since the scan keeps one observation per sample or boundary rather than one per frame.

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
- `inspect <work> [--step <step>]` prints a job's compact occurrence readings, translations,
  confidence and review flags from its database (`text_typeset` unless `--step` names another
  on-screen text step).
- `scenarios` creates an eight-second Japanese source fixture and independent annotations for
  scrolling credits, fading lyrics, vertical writing, a brief sign, a cut and repeated writing.
- `evaluate <work> <annotations> <verdict>` checks a pilot job's typeset text, read from its
  database, against the annotations, writes a machine-readable verdict and exits unsuccessfully
  when acceptance fails.
- `mask-probe <work> <id>…` prints, for occurrences of a finished job, every figure the stroke-mask
  step judges them by (rectangles, line height, the sampled quads and their offset from the
  keyframe quad, each colour partition's reading, coverage, cut share, largest piece and failed
  guard, completion counts, style, refitted area, for moving writing every frame's match and
  whether it counts as still, and the verdict),
  through the production `replace::mask::diagnose`, decoding regions of the job's source video on
  the CPU. `--ruby l,t,r,b` stands in furigana boxes, `--out` writes each keyframe plate and its
  tinted mask, and `--all` reruns the whole step into `--out` and prints each verdict beside the
  job's with the mask's overlap.
- `residue-probe <work>` prints how much of each filled plate's mask still looks like the
  lettering, through the production `replace::inpaint::residue_share`.
- `verify-probe <work> [<id>…]` runs the production read-back check (`replace::verify::verify`)
  over a finished job's composed replacements and `frames` rows, read from its database, on the
  host GPU and prints, per sampled frame,
  the region, lettering and line height read, every line found with its box score, reading,
  confidence and whether it counts as the English (E) or where the writing was (J), the
  similarity and the verdict; then each occurrence's result and the similarity distribution.
  `--out` saves each finished region read as a PNG. It needs the CUDA and ONNX Runtime libraries
  on `LD_LIBRARY_PATH` and `ORT_DYLIB_PATH`, as a GPU worker has them.
- `font-candidate` inspects an official Google Fonts candidate before checksum pinning.

## Boundaries

- Depends on: the production model, stage, media, pipeline and subtitle crates.
- Used by: development validation on owner-provided media.
- Rules: no models are converted, missing readable occurrences fail, and unsafe tracks need an explicit fallback.

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md) — host execution.
