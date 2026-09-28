# Bundled FFmpeg

The static FFmpeg the AppImage carries: BtbN's n8.1.3 linux64 GPL build, pinned by URL and
SHA-256, copied to `usr/bin/ffmpeg/`, and checked by running it.

## Contents

```text
tools/appimage_builder/src/ffmpeg/
├── mod.rs  the pin, `fetch`, `bundle` and `verify`: version 8.1 or newer, `scdet`, `apad` and `pulse`
└── tests/  version parsing, the filter and device listings, and the pin's shape
```

## Boundaries

- Depends on: `inference::model_store` (the hash-checked download and unpack),
  `child_process::Run` (running the copied programs), `gpu_runtime::print_progress`.
- Used by: `main.rs`: `fetch` and `bundle` while laying out the AppDir, `verify` in the smoke
  check.
- Rules:
  - the archive's SHA-256 matches the pin before it is unpacked (`install_archive`);
  - a build older than 8.1, or without `scdet`, `apad` or the `pulse` output device the clip
    player uses, stops the run (`verify`; `finds_a_filter_or_device_by_its_row_name`).
