# Log console services

The log window's words, with no rendering code: the time, a group's header and a line as the
window writes them, a model call's summary and place, and the text Copy and Copy All hand over.

## Contents

```text
apps/tbd_subtitles/src/log_console/services/
├── call_text.rs     `summary`, `place` and `call_text`: a model call's line and the whole call
├── console_text.rs  `time`, `step_words`, `header`, `line_text` and `copy_text`
├── mod.rs           the module list
└── tests/           unit tests of the time, headers, copied lines and a copied call
```

## How it works

`time` writes the time since the window opened as `01:23.456`, or `1:02:03.4` from the first hour
on, nine characters wide. `step_words` names a job step by its stage and plain title from
`crate::core::steps` ("Settle the words — Language model settles the words"), and `fix_it` as
"Fix It"; `header` puts the video before it. `line_text` writes a line in full for Copy and the
detail panel: time, level, writer, source, where, message; `copy_text` writes every shown line.
`summary` gives a call's seconds, tokens and cost; `call_text` writes the whole call for Copy
All: its id, model, purpose, place and outcome, then the system prompt, the message, the schema
and the answer under headings, in the order they were sent.

## Boundaries

- Depends on: `crate::core::log_buffer` (`LogLine`, `KeptCall`), `crate::core::steps`,
  `crate::log_console::models`, `job_model::StepName` and `tracing` (`Level`).
- Used by: `crate::log_console::ui`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).
