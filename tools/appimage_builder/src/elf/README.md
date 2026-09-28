# ELF dynamic sections

What the builder needs from ELF files: the NEEDED, SONAME and RUNPATH entries, the closure of
libraries a binary needs, copies of a library with its soname symlinks, and a RUNPATH rewritten in
place.

## Contents

```text
tools/appimage_builder/src/elf/
├── closure.rs       the libraries a set of files needs, found as the loader does; the host's libraries skipped
├── library_copy.rs  a library copied by name with its chain of soname symlinks
├── mod.rs           `Dynamic` and `read_dynamic`: the dynamic entries of one file, read with `object`
├── runpath.rs       a RUNPATH or RPATH overwritten in `.dynstr` with a shorter one, NUL padded
└── tests/           a hand-built tiny ELF library, parsing, closure, copies and the rewrite
```

## How it works

`mod.rs` parses the section headers with `object`, takes the `SHT_DYNAMIC` section and its linked
string table, and records each NEEDED name, the SONAME, and the RUNPATH text with its byte offset
in the file. `closure.rs` walks NEEDED breadth first from the roots, resolving each name in the
needing file's RUNPATH (`$ORIGIN` expanded) and then in the given folders; the host's libraries
(glibc, the GCC runtimes, zlib, the NVIDIA driver) and names another step provides are skipped,
and a name found nowhere is an error. `runpath.rs` writes the new text over the old at that offset,
so the file's size and layout never change.

## Boundaries

- Depends on: `object` (ELF reading), `anyhow`, the standard library's file system.
- Used by: `main.rs` (the app's host-only check and the ggml worker's libraries),
  `gpu_runtime` (the runtime libraries' closure and copies) and `build` (the CrispASR check).
- Rules:
  - a new RUNPATH longer than the old is refused and the file is left as it was
    (`rewrites_a_runpath_in_place_and_refuses_a_longer_one`);
  - host and provided libraries are never walked, and an unresolved name is an error
    (`closure_follows_runpath_and_skips_host_and_provided_libraries`);
  - a copy keeps every soname symlink (`copies_a_library_with_its_symlink_chain`).
