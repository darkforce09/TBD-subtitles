# Release build

The three release builds the AppImage packs: `tbd-subtitles`; its Whisper worker
`tbd-subtitles-ggml` with the `crispasr` feature under the CUDA 13.4 toolkit in the runtime
folder; and its local translation worker `tbd-subtitles-llm` with the `mistralrs` feature,
compiled by the CUDA 13.3 compiler in the runtime folder and linked to the CUDA 13.4 libraries,
as the development environment runbook builds them by hand.

## Contents

```text
tools/appimage_builder/src/build/
└── mod.rs  `build`: the three cargo runs, their CUDA environments and each worker's GPU backend check
```

## Boundaries

- Depends on: `child_process::Run` (for `cargo`), `inference::model_store::manifest::CUDA_FOLDER`
  and `CUDA_BUILD_FOLDER`, and `elf::read_dynamic`.
- Used by: `main.rs`, after the runtime archives are unpacked.
- Rules:
  - a ggml worker that does not NEED `libcrispasr.so.1` stops the run: it would refuse every
    Whisper step;
  - a local worker that does not NEED `libcublas.so.13` and `libcudart.so.13` stops the run: it
    was built without the mistral.rs CUDA backend;
  - `--skip-build` still checks that all three binaries exist.

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md) — steps 12 and 13,
  the same builds by hand.
