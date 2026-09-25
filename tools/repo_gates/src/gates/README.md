# Gates

Every gate of `cargo gates`, grouped by what it judges: documents, source and text files, and the
Cargo workspace. `mod.rs` maps each `Gate` name to the code that runs it.

## Contents

```text
tools/repo_gates/src/gates/
├── documentation/  README coverage and sections, Markdown placement, status lines and the link check
├── mod.rs          `run`: dispatches one `Gate` to its module and returns its exit status
├── source/         languages, file length, module headers, test placement, prose rules, whitespace
└── workspace/      crate layering over the workspace's `Cargo.toml` manifests
```

## How it works

`run` in `mod.rs` takes one `Gate`, the workspace root, the `GateRequest` and the `--report`
flag, and returns 0, 1 or 2. Two shapes of gate exist:

- A file rule implements `gate_run::file_rule::FileRule`: it names the tracked files it selects
  and returns the problems it finds in each file's bytes; `verify_file_rule` does the listing,
  scoping, reading and reporting. Every gate under `source/` and `workspace/`, README sections and
  status lines take this shape.
- A whole-tree gate drives `gate_run::prepare` itself because it judges something other than one
  file at a time: README coverage judges folders against their READMEs, Markdown placement judges
  where documents sit, and the link check scans every judged document once and runs several rules
  over the scan.

```text
gates::run(gate)
   ├── verify_file_rule(&Rule)        language-bans, file-length, module-headers, test-placement,
   │                                  prose-rules, editorconfig, crate-layering, readme-sections,
   │                                  status-lines
   ├── verify_readme_coverage         readme-coverage
   ├── verify_markdown_placement      markdown-placement
   └── verify_link_check(listing)     link-check (--report prints every break)
```

A new gate adds a `Gate` variant in `tools/repo_gates/src/cli.rs`, a module in the group it
belongs to, and one arm in `run`.

## Public surface

- `gates::run(gate, repo_root, request, report) -> u8`, called by `main.rs` for each chosen gate.
- Nothing else leaves the folder: every gate module is `pub(crate)` for `run` and its tests.

## Boundaries

- Depends on: `tools/repo_gates/src/gate_run/` (listing, scope, regions, run report);
  `tools/repo_gates/src/layout.rs`; `tools/repo_gates/src/cli.rs` (`Gate`, and the command tree
  the link check walks); `verification_core`; `regex`, `syn`, `toml`; the `git` program.
- Used by: `tools/repo_gates/src/main.rs`.
- Rules:
  - every `Gate` variant has exactly one arm in `run`, which the exhaustive `match` holds at
    compile time;
  - a gate that could not examine its input reports "did not run", never a pass
    (`an_unreadable_file_or_nothing_selected_did_not_run`, `a_failed_or_empty_listing_did_not_run`,
    `an_unreadable_document_or_a_failed_listing_did_not_run`).

## Related documentation

- [Gate runner source](/tools/repo_gates/src/README.md#commands) — each gate's name and the law
  it holds.
- [Coding standards](/documentation/standards/coding_standards.md) — the source laws.
- [README standard](/documentation/standards/readme_standard.md) — the README laws.
