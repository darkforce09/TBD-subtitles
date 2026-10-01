# Workspace gates

The gate over the Cargo workspace as a whole: `crate-layering`, which keeps every crate's
dependencies pointing down the layer table.

## Contents

```text
tools/repo_gates/src/gates/workspace/
├── crate_layering.rs  `crate-layering`: each manifest's workspace dependencies against the layer table
├── mod.rs             the module tree of the workspace gates
└── tests/             unit tests over fixture manifests and this workspace's own
```

## How it works

`crate-layering` is a `FileRule` over every `Cargo.toml` in the code trees. It parses the manifest
with `toml`, reads the package name, and looks the crate up in `tools/repo_gates/src/layout.rs`:

- a product crate (`PRODUCT_LAYERS`) may depend only on product crates of a strictly lower layer:
  `job_model`, `child_process`, `app_icon` and `worker_channel` at 0; `media_io`,
  `subtitle_formats` and `inference` at 1; `stages` at 2; `pipeline` at 3; the three app
  binaries `tbd_subtitles`, `tbd_subtitles_ggml` and `tbd_subtitles_llm` at 4;
- a tool (`TOOL_DEPENDENCIES`) may depend only on the workspace crates listed for it, such as
  `verification_core` on `child_process`, `repo_gates` on `verification_core`, and
  `redb_process_probe` on none; the table holds every tool under `tools/`;
- a crate in neither table fails, so a new crate is placed before it passes.

Every dependency table counts (normal, dev and build, `[target.*]` tables included), and a
`package =` rename is followed to the real crate name. Crates from crates.io are not judged.

## Boundaries

- Depends on: `tools/repo_gates/src/gate_run/` (`FileRule`, `path_regions`);
  `tools/repo_gates/src/layout.rs` (the layer and tool tables); `toml`.
- Used by: `tools/repo_gates/src/gates/mod.rs`, for `crate-layering`.
- Rules:
  - a crate depends only on lower layers, or on its listed crates for a tool
    (`a_crate_may_depend_on_lower_layers_only`,
    `tools_use_only_their_listed_crates_and_unknown_crates_fail`);
  - renames and target tables cannot hide a dependency (`renames_and_target_tables_are_followed`);
  - this workspace holds its layers (`the_workspace_holds_its_layers`).

## Related documentation

- [Coding standards](/documentation/standards/coding_standards.md#layering) — the layering law
  this gate holds.
