**Status:** live

# Template: decision entry

**When to use:** one decision in the project's decision log,
[decisions](/documentation/decisions/): a choice that shapes the app and would otherwise be
argued again. An entry says what was decided, why, what follows from it, and which earlier entry it
replaces. Open questions not yet decided stay in the roadmap's open questions table until they are
settled. Live documents carry no dates, decision entries excepted. The
[documentation standards](/documentation/standards/documentation_standards.md) fix the entry's
parts.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. Each entry is a
`###` heading at the end of the log's file for its subject, with its four parts in this order.

````markdown
### <YYYY-MM-DD> — <the decision, as a short statement>

**Context:** <the problem, the constraints, and the options on the table when it was decided>

**Decision:** <what was decided, precisely enough to check the code against it>

**Consequences:** <what follows: what the code must do, what it rules out, the costs accepted, and
the gate or test that holds the decision in place>

**Supersedes:** <the date and heading of the entry this one replaces, as that heading spells them;
or "none.">
````

The date is the day the decision was taken. An entry is never reworded once written: when a
decision changes, a new entry records the new one and names the old entry under Supersedes, so the
log shows which decision holds. Context describes the situation at that date. The decision log is
the one document that may tell how things changed, so the history words `cargo gates prose-rules`
bans elsewhere are allowed in it; the ticket ids it bans everywhere are not. A heading's anchor is
its text in lowercase with the spaces turned into hyphens and the dash dropped, which is how other
documents link an entry:

```text
[Each GPU stage runs in its own worker process](/documentation/decisions/foundations.md#2026-09-25--each-gpu-stage-runs-in-its-own-worker-process)
```

## Worked sample

Copied from the decision log's entry on native inference runtimes, the one entry that supersedes
another, and checked against the inference crate's header in `crates/inference/src/lib.rs`, which
states the ggml invariant. The sample sits in a fenced block, so no gate reads it.

````markdown
### 2026-09-25 — Native inference runtimes allowed where no pure-Rust engine competes

**Context:** The earlier entry allowed ONNX Runtime (through `ort`) and ggml (through whisper-rs,
transcribe-cpp or crispasr) pending the owner's confirmation. The owner confirmed it.

**Decision:** Inference uses pure-Rust engines (candle, burn, mistral.rs, earshot) first. Rust
crates that bind ONNX Runtime or ggml are used where no pure-Rust option is competitive in speed or
accuracy. All project code stays Rust.

**Consequences:** Parakeet ONNX, MDX-Net and Mel-Band RoFormer separation on CUDA, CED sound events
and the Qwen3 aligner through crispasr are all open to the M0.5 spike. CUDA 13 runtime libraries
ship beside the app, and each GPU stage keeps its own worker process so native libraries never
share a binary.

**Supersedes:** 2026-09-25 — Native inference runtimes through Rust crates are allowed (to confirm).
````
