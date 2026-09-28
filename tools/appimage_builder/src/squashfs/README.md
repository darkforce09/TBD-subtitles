# Squashfs image

The AppDir written as a zstd-compressed squashfs image with `backhand`, at an offset in the output
file so the AppImage runtime's bytes come first.

## Contents

```text
tools/appimage_builder/src/squashfs/
├── mod.rs  `write_image`: the sorted walk of the AppDir, modes and symlinks kept, owner root
└── tests/  an image written after a head and read back: contents, mode, owner and symlink
```

## Boundaries

- Depends on: `backhand` (with its `zstd` feature), `anyhow`, the standard library's file system.
- Used by: `main.rs`, after the smoke check.
- Rules:
  - entries keep their mode, symlinks stay symlinks and every owner is root
    (`writes_an_image_after_a_head_that_reads_back_with_modes_and_links`);
  - a device, fifo or socket in the AppDir is an error.
