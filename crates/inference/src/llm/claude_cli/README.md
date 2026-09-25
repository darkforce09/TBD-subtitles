# Claude CLI backend

The headless `claude -p` CLI as a language-model backend: the owner's Claude subscription answers
in JSON that matches a schema, with every tool disabled.

## Contents

```text
crates/inference/src/llm/claude_cli/
├── mod.rs  `ClaudeCli` (program, model, deadline, working folder) and `parse` of the JSON result
└── tests/  unit tests for reading the structured answer, tokens and cost, and for error results
```

## Boundaries

- Depends on: `child_process::Run` (the `claude` program with a deadline), `serde_json`.
- Used by: `tools/stack_spike/` (through `stages::adjudication`).
- Rules:
  - it runs `claude -p --output-format json --json-schema … --tools "" --no-session-persistence
    --strict-mcp-config --disable-slash-commands --setting-sources project` in an empty folder, so
    the owner's user-level hooks, plugins and MCP servers never reach the prompt (review);
  - a result without `structured_output`, or marked `is_error`, is an error
    (`a_result_without_structured_output_is_an_error`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#6-language-models) — the CLI flags.
