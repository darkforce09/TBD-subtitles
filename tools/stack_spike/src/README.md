# Stack spike source

The `stack-spike` command line and one module per command.

## Contents

```text
tools/stack_spike/src/
├── fetch.rs  the `fetch` command: list, download and check the pinned models and runtime archives
└── main.rs   the binary root: the command line and the dispatch to each command
```

## Boundaries

- Depends on: `inference::model_store`; `clap` for the command line, `anyhow` for errors.
- Used by: nothing; it is the binary's source.
- Rules: a command that could not run exits non-zero with the reason, never a success (the
  header in `main.rs`).
