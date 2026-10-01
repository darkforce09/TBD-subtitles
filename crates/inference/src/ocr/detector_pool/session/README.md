# Detector session

One PP-OCRv5 detector session of the pool: what it is opened with, the execution providers and
their options, the TensorRT engine cache, the ONNX Runtime session with its bound buffers, and the
warm-up with its one reopen.

## Contents

```text
crates/inference/src/ocr/detector_pool/session/
├── engine_cache.rs  the TensorRT cache key and folder, profile shapes, workspace size, identity check
├── mod.rs           `SessionSpec`, `CudaTuning`, the `DetectorSession` and `SessionOpener` traits, `open_warm`
├── onnx_input.rs    the model's input name, read from the ONNX file's protobuf encoding
├── ort_detector.rs  `OrtOpener`: the ONNX Runtime session, `IoBinding`, pinned and device tensors
├── providers.rs     the TensorRT and CUDA providers with every option, in registration order
└── tests/           fake openers: warm-up, the reopen, cache keys, profiles, provider options
```

## How it works

`open_warm` opens a session through a `SessionOpener` with `CudaTuning::FULL`, fills its staging
buffer with black frames and runs it `WARMUP_RUNS` times: the first run makes the convolution
search or builds the engine, the second captures the CUDA graph. If opening or warming up fails,
the session is dropped and reopened once with `CudaTuning::PLAIN` (no CUDA graph, no NHWC); the
notes record which way it runs and the refusal's error, and a second failure is an error naming
both.

`OrtOpener` builds the session with graph optimisation level 3, one intra-op and one inter-op
thread, spinning off, no environment providers and deterministic kernels only in
`SearchMode::Deterministic`. `providers.rs` gives the CUDA provider the session's memory pool
(`SameAsRequested` growth), TF32, NHWC and the CUDA graph, with exhaustive cuDNN search and the
largest workspace in the fast mode and heuristic search in the deterministic mode. For TensorRT it
registers the TensorRT provider first (FP16 unless the bench asks for FP32, engine and timing
caches, one optimisation profile whose minimum, optimum and maximum are the session's shape,
the workspace `engine_cache::workspace_mib` leaves within the worker's cap) and the CUDA provider
second, both failing the session when they cannot register. The CUDA graph is then TensorRT's
own, since ONNX Runtime captures one only when a single provider runs every node.

Before a TensorRT session opens, `ort_detector.rs` reads the model file once: `onnx_input.rs`
finds its input name for the profile, and its SHA-256 goes into the cache key with the GPU name,
driver and TensorRT version of `EngineIdentity`, the input shape and the precision. The engine and
timing caches live in `<cache root>/<key>/`; a folder without an `.engine` file means this session
builds the engine, and its opening and warm-up count as engine build time. After opening, the
session's own input name must equal the file's.

The session binds a CUDA device input tensor and a CUDA device output tensor once. Each run
copies the pinned staging tensor into the device input with `copy_into`, so neither bound
address ever changes, runs the binding, and copies the device output into a host tensor with
`copy_into`, from which the probability maps are read. ONNX Runtime refuses an output bound in
`CUDA_PINNED` memory (its copy of the output lands on the output itself and fails), and a
captured CUDA graph needs both bound tensors on the device.
`copy_into` runs through ONNX Runtime's shared copy session, so copies from the pool's threads
take turns.

## Boundaries

- Depends on: `ort` (sessions, the CUDA and TensorRT providers, `IoBinding`, allocators,
  tensors), `sha2`, `job_model` (`DetectorEngine`), the pool's `batch.rs` (`InputShape`,
  `fill_black`) and `SearchMode`, and the `pool` contract's `EngineIdentity`.
- Used by: the pool's `mod.rs` (specs, `OrtOpener`) and `worker.rs` (`open_warm`).
- Rules:
  - TensorRT registers before CUDA and every provider fails loudly (`tests/providers.rs`).
  - Any change of GPU, driver, TensorRT version, model, shape or precision gives a new cache key,
    and an identity with an empty field is refused (`tests/engine_cache.rs`).
  - A refused CUDA graph or NHWC reopens exactly once and shows in the notes
    (`tests/session.rs`).
