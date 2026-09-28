# Claude CLI backend

The headless `claude -p` CLI as a language-model backend: the owner's Claude subscription answers
in JSON that matches a schema, with every tool disabled.

## Contents

```text
crates/inference/src/llm/claude_cli/
├── mod.rs  `ClaudeCli` (program, model, deadline, working folder, cancel flag) and `parse`
└── tests/  unit tests for the structured answer, tokens and cost, error results and cancelling
```

## Boundaries

- Depends on: `child_process::Run` (the `claude` program with a deadline), `serde_json`, and
  `tracing` for one info line per call (model, input lines, seconds, tokens, cost; a failure as a
  warning), never the prompt or the answer.
- Used by: `crates/pipeline/` (the adjudication tasks and Fix It) and `tools/stack_spike/`
  (through `stages::adjudication`).
- Rules:
  - it runs `claude -p --output-format json --json-schema … --tools "" --no-session-persistence
    --strict-mcp-config --disable-slash-commands --setting-sources project` in an empty folder, so
    the owner's user-level hooks, plugins and MCP servers never reach the prompt (review);
  - a result without `structured_output`, or marked `is_error`, is an error
    (`a_result_without_structured_output_is_an_error`);
  - with a cancel flag (`with_cancel`) the call runs under the child's watchdog, which kills
    `claude` and its process group once the flag is set; the call then fails as `cancelled`
    (`a_set_cancel_flag_stops_the_call`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#6-language-models) — the CLI flags.
