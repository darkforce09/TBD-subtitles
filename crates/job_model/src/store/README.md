# Job database layout

What a job's database keeps about its own tables: the layout version of the types each table was
written with, so a table written with an older layout is dropped instead of read wrongly.

## Contents

```text
crates/job_model/src/store/
├── mod.rs  `TableLayouts`, the layout version of every table by table name
└── tests/  the rkyv round trip of the layouts
```

## Boundaries

- Depends on: `serde` and `rkyv` for the derives.
- Used by: `crates/pipeline/src/work_dir/store/`, which stores it under the key `"layout"` of the
  `meta` table and compares it with the versions the code declares on every open.
- Rules: the layouts round-trip through rkyv from a misaligned slice
  (`table_layouts_round_trip` in `tests/archive.rs`).

## Related documentation

- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#adding-a-field) — why
  each table carries a layout version and what a changed version does.
