# App icon source

The `app_icon` library: one file that paints the icon's pixels, and its unit tests.

## Contents

```text
crates/app_icon/src/
├── lib.rs  `rgba`: the rounded tile and the two subtitle bars, 4×4 supersampled, as RGBA8 pixels
└── tests/  unit tests for the pixel count, the transparent corners and repeatability
```

## Public surface

- `rgba(size: u32) -> Vec<u8>`, for `tools/appimage_builder/src/app_dir/icon.rs` and the app's
  window in `apps/tbd_subtitles/src/application/mod.rs`.

## Boundaries

- Depends on: the standard library alone.
- Used by: `tools/appimage_builder/src/app_dir/icon.rs` and
  `apps/tbd_subtitles/src/application/mod.rs`.
- Rules: the corners are fully transparent and the same size gives the same bytes
  (`the_corners_are_transparent_and_the_centre_opaque`, `the_same_size_gives_the_same_pixels` in
  `tests/rgba.rs`).
