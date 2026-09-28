# Release build

The two release builds the AppImage packs: `tbd-subtitles`, then its Whisper worker
`tbd-subtitles-ggml` with the `crispasr` feature under the CUDA 13.4 toolkit in the runtime
folder, as the development environment runbook builds them by hand.

## Contents

```text
tools/appimage_builder/src/build/
└── mod.rs  `build`: the two cargo runs, their CUDA environment and the check that the worker links CrispASR
```

## Boundaries

- Depends on: `child_process::Run` (for `cargo`), `inference::model_store::manifest::CUDA_FOLDER`,
  and `elf::read_dynamic`.
- Used by: `main.rs`, after the runtime archives are unpacked.
- Rules:
  - a worker that does not NEED `libcrispasr.so.1` stops the run: it would refuse every Whisper
    step;
  - `--skip-build` still checks that both binaries exist.

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md) — steps 12 and 13,
  the same builds by hand.
