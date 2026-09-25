**Status:** live

# Template: research snapshot

**When to use:** a dated survey of the options for the capabilities the app needs (crates, model
files, runtimes, benchmarks, prices), written once and then kept as it stands. Snapshots live in
`documentation/research/`, one file each, named in snake_case after the subject; a snapshot that
follows an earlier one on the same subject adds its date to the name
(`rust_ml_stack_2026_10_12.md`). Each is listed in the research folder's README. The
[documentation standards](/documentation/standards/documentation_standards.md) define frozen
records and their lifecycle.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. The status line
is `**Status:** frozen record (YYYY-MM-DD)` with the day the facts were gathered: a snapshot is a
frozen record from its first commit. Capability sections are numbered in the order the pipeline
uses them; a capability with too few options for a table gives them in one paragraph instead.

````markdown
**Status:** frozen record (<YYYY-MM-DD>)

# <Subject of the snapshot, in plain words>

<What the snapshot surveys, the date, where the facts come from (registries, repositories, model
hubs, benchmarks, tool versions checked), and what each marker in the tables means, such as
"(C++)" for a Rust crate that binds a C or C++ runtime. Link the decision entries the survey
serves.>

## 1. <Capability>

| Option | <Version, date> | <the property that decides, such as GPU support> | <Notes> |
|---|---|---|---|
| [<crate or model>](<its repository or model page>) (<runtime marker>) | <version, release date> | <value> | <what matters for this app: limits, speed, fit in the GPU budget> |

<Published measurements, options ruled out and why, and anything a table cell cannot hold.>

**Recommendation:** <the option to use, the settings or chunking it needs, the fallback, and
what must be measured before relying on it>

## Recommended stack

| Capability | Choice |
|---|---|
| <Capability> | <the recommended option, with its alternative in brackets> |

## Hard gaps

- **<gap>:** <what no option covers, and how the app works around it>

## Sources

[<source title>](<URL>) · [<source title>](<URL>)
````

A snapshot keeps its words. Only a broken link is fixed; new facts (a new release, a measurement
on real audio) go into a new snapshot, and a decision that changes because of them gets a new
entry in the [decision log](/documentation/decisions.md). A frozen record carries dates freely,
may pass 500 lines, and is judged only for its status line and its links:
`cargo gates status-lines` requires the frozen status in the research folder, and
`cargo gates link-check` checks its links but not its backticked paths or cited commands.

## Worked sample

Abridged from `documentation/research/rust_ml_stack.md`: its introduction, two of its twelve
capability sections with their numbers kept, the matching rows of its recommended stack, one hard
gap and one source. The sample sits in a fenced block, so no gate reads it.

````markdown
**Status:** frozen record (2026-09-25)

# Rust ML stack

The Rust crates and ready-made model files that can run each capability the app needs, as found
on 2026-09-25 (crates.io, GitHub, Hugging Face; Claude CLI flags checked against v2.1.282). "(C++)"
marks a Rust crate that binds a C/C++ runtime; see the
[native-runtime decision](/documentation/decisions.md). Nothing here needs Python, and every model
file named is already exported. Versions and dates are from that day.

## 2. Voice activity detection

| Option | Version, date | F1 / cost per second of audio |
|---|---|---|
| [earshot](https://github.com/pykeio/earshot) (pure Rust) | 1.2.2, 2026-08-19 | 0.928 / 0.0003 |
| [silero](https://github.com/Findit-AI/silero) (ort) | 0.7.0, 2026-08-22 | 0.938 / 0.002 |
| [ten-vad-rs](https://github.com/wangfu91/ten-vad-rs) (ort) | 0.1.7, 2026-04-03 | 0.928 / 0.002 |

Benchmark: [wavekat-vad](https://github.com/wavekat/wavekat-vad). Avoid voice_activity_detector
(pinned to an old `ort`) and silero-vad-rs (unmaintained).

**Recommendation:** earshot on the vocal stem, Silero for borderline frames; pad 200 ms, merge
gaps under 300 ms. Every detector fires on songs, so songs are labelled by sound events.

## 5. Sound events

| Option | Notes |
|---|---|
| [soundevents](https://github.com/findit-studio/soundevents) 0.5.0 (ort) | CED tiny to base ONNX bundled; AudioSet mAP 48.1–50.0; 527 typed labels; chunking built in |
| [AST ONNX](https://huggingface.co/Xenova/ast-finetuned-audioset-10-10-0.4593) + ort | mAP 45.9; needs Kaldi filterbank features |
| [CLAP ONNX](https://huggingface.co/Xenova/larger_clap_general) + ort | zero-shot: labels given as text |
| SenseVoice (sherpa-onnx or crispasr) | emits laughter, applause, crying, cough, music tags inline |

PANNs, BEATs and EfficientAT have no maintained ONNX export; YAMNet is weak.

**Recommendation:** CED-base over 2 s windows every 0.5 s on the mix and the background stem,
mapped to SDH cues with per-class thresholds and smoothing; CLAP for sounds AudioSet lacks.

## Recommended stack

| Capability | Choice |
|---|---|
| Voice activity | earshot, Silero as backup |
| Sound events | soundevents (CED-base) + CLAP ONNX |

## Hard gaps

- **Frame-level sound-event models:** none exported; CED over sliding windows instead.

## Sources

[CED](https://huggingface.co/mispeech/ced-base)
````
