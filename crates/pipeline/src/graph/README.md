# Stage graph

The stage graph: which [stages](/documentation/glossary.md#stage) run, in which order, on which
inputs. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/pipeline/src/graph/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/pipeline/src/lib.rs` declares it as a public module.
- Rules: the run order is the one `StageName::ALL` gives in
  `crates/job_model/src/stage/stage_name.rs`, which its tests hold.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow and its inputs.
