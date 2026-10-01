# Visual validation source

Commands and fail-closed evaluation for local visual pilots.

## Contents

```text
tools/visual_validation/src/
├── evaluate.rs      coverage, timing and geometry verdicts on a pilot job's typeset text
├── job_rows.rs      a finished job's source, probe and documents, read from its database
├── main.rs          fetch, still recognition, clip, evaluation and probe commands
├── mask_probe.rs    stroke-mask and residue diagnostics on a finished job, and a whole-step rerun
├── pilot.rs         measured six-stage pilots using production workers and resume
├── scenarios.rs     synthetic Japanese credits, lyric, vertical and brief-text source fixtures
├── verify_probe.rs  the read-back check over a finished job, every frame's readings printed
└── tests/           evaluator regression cases
```

## How it works

Commands use production backends and write explicit outputs. Evaluation matches independent annotations to distinct observed occurrences and checks timing and geometry at source resolution. The mask and residue probes call the production `replace::mask::diagnose`, `replace::mask::extract` and `replace::inpaint::residue_share` on a finished job's documents, read from its database, and its files and source video, on the CPU, and write only under the folder they are given. The verify probe runs the production `replace::verify::verify` with PP-OCRv5 on the GPU over the same documents and files and writes only the regions it read, under `--out`.

## Boundaries

- Depends on: shared Rust application crates.
- Used by: the visual validation command.
- Rules: source videos are read-only and annotation failures never become passing verdicts.

## Related documentation

- [Validation tool](/tools/visual_validation/) — commands and configuration.
