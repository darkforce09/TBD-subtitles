# App icon

The `app_icon` crate: the TBD-subtitles icon, a rounded dark tile with two white subtitle bars,
painted in code as RGBA pixels at any square size. Every binary that shows the icon takes its
pixels from here, so the AppImage's icon and the app's own are the same picture. It depends on no
crate at all.

## Contents

```text
crates/app_icon/
├── Cargo.toml  the `app_icon` library package, with no dependencies
└── src/        the painting: the tile, the two bars and the supersampled edges
```

## How it works

`rgba(size)` walks the `size`×`size` pixels row by row and takes 4×4 samples in each. A sample
takes the colour of the topmost shape it falls in (the lower bar, the upper bar, then the tile) or
none outside the tile; a pixel's alpha is the share of its samples inside the tile and its colour
the mean of their colours, so the rounded edges are smooth and the corners fully transparent. The
pixels are straight RGBA8, not premultiplied, which is what a PNG encoder and a window icon take.

## Getting started

Run these from the repository root:

```bash
cargo build -p app_icon   # the library
cargo test -p app_icon    # 3 unit tests for the size, the corners and repeatability, well under a second
```

## Configuration

None: the crate reads no setting.

## Public surface

- `app_icon::rgba(size: u32) -> Vec<u8>`: the icon as `size * size * 4` bytes of straight RGBA8,
  row-major from the top-left corner.
- No binary.

## Boundaries

- Depends on: the standard library alone. No workspace crate and no crates.io crate.
- Used by: `tools/appimage_builder/src/app_dir/icon.rs`, which encodes the pixels as the AppDir's
  PNG.
- Rules:
  - the crate sits in layer 0 and depends on no workspace crate (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - the corners are transparent and the same size always gives the same bytes
    (`the_corners_are_transparent_and_the_centre_opaque` and
    `the_same_size_gives_the_same_pixels` in `crates/app_icon/src/tests/rgba.rs`);
  - a change to the picture changes the AppImage's icon, whose PNG bytes a builder test pins
    (`the_icon_png_bytes_match_the_shipped_icon` in
    `tools/appimage_builder/src/app_dir/tests/app_dir.rs`).

## Related documentation

- [Building the AppImage](/documentation/runbooks/building_the_appimage.md) — where the icon goes
  in the AppImage and on the desktop.
