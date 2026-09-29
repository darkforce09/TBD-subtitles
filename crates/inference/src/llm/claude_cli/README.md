# Claude CLI backend

The headless `claude -p` CLI as a language-model backend: the owner's Claude subscription answers
in JSON that matches a schema, with every tool disabled.

## Contents

```text
crates/inference/src/llm/claude_cli/
├── mod.rs           `ClaudeCli`, its text and image requests, and `parse`
├── shared_slots.rs  cancellable file-lock permits shared by every Claude caller
└── tests/           unit tests for answers, tokens and cost, errors, image messages and cancelling
```

## Boundaries

- Depends on: `child_process::Run` (the `claude` program with a deadline), `serde_json`, and
  `super::call_log`, which logs each call: its summary line, and the whole exchange with what
  `claude` printed, for the app's log window.
- Used by: `crates/pipeline/` (the adjudication tasks and Fix It), `tools/stack_spike/`
  (through `stages::adjudication`), and the visual translation stage in `stages::onscreen_text`,
  whose crop requests (`complete_image_json`) and keyframe requests (`complete_images_json`) send
  PNG images.
- Rules:
  - it runs `claude -p --output-format json --json-schema … --tools "" --no-session-persistence
    --strict-mcp-config --disable-slash-commands --setting-sources project` in an empty folder, so
    the owner's user-level hooks, plugins and MCP servers never reach the prompt (review);
  - an image request keeps that isolation, switches to `--output-format stream-json
    --input-format stream-json --verbose` and sends one user message: the text, then each PNG in
    order (`several_images_follow_the_text_in_one_user_message`); with no image it is the plain
    text call (`an_empty_image_list_makes_the_plain_text_call`);
  - a result without `structured_output`, or marked `is_error`, is an error
    (`a_result_without_structured_output_is_an_error`);
  - with a cancel flag (`with_cancel`) the call runs under the child's watchdog, which kills
    `claude` and its process group once the flag is set; the call then fails as `cancelled`
    (`a_set_cancel_flag_stops_the_call`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#6-language-models) — the CLI flags.
