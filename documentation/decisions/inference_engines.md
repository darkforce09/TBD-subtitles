**Status:** live

# Decisions: inference engines

The decisions about which inference engine runs a model on the GPU beyond the runtimes the
[foundations](/documentation/decisions/foundations.md) allow, how those engines are packaged, and
when a downloaded model may be converted or compiled. The [decision log](/documentation/decisions/)
says how entries are written; the runtimes themselves are pinned in
`crates/inference/src/model_store/manifest/gpu_runtime.rs` and packaged by
`tools/appimage_builder`.

### 2026-10-01 — TensorRT runs the PP-OCRv5 detectors

**Context:** Full-resolution screening runs the PP-OCRv5 mobile detector on every sampled frame
at 1920×1088. The detect-bench (commit 5febb2b) measured it GPU-bound near 35 frames per second
with one CUDA session on Dressrosa 11 and 28, so the detector, not decoding, sets the pace of
`text_detect`. Law 4 allows a native runtime through a Rust crate where no pure-Rust option is
competitive, which is why ONNX Runtime already runs these detectors on CUDA. ONNX Runtime's
TensorRT execution provider (the `ort` feature `tensorrt`) fuses layers, tunes kernels for the
RTX 3070 and builds FP16 engines from the FP32 ONNX files already downloaded. ONNX Runtime
1.28.2's CUDA 13 build is made against TensorRT 10.14.1.48 for CUDA 13.0: its provider links
`libnvinfer.so.10` and `libnvonnxparser.so.10` and opens `libnvinfer_plugin.so.10` and
`libnvinfer_builder_resource.so.10.14.1` at run time, so TensorRT 11 will not load. No published
FP16 export of the PP-OCRv5 mobile detector was found: the pinned oar-ocr v0.3.0 release assets,
RapidOCR's models and other mirrors were checked. NVIDIA's TensorRT licence (SLA §12.1)
lets an application distribute only "the libnvinfer and libnvinfer_plugin libraries", as part of
an application with material additional functionality and never on their own; `libnvonnxparser`
and the builder resource are not named.

**Decision:** The screening and confirmation detectors can run on TensorRT through ONNX Runtime's
TensorRT provider, registered ahead of the CUDA provider, with FP16 on. Engines and timing caches
live under `<app data>/tensorrt/`, keyed by the GPU name, the driver, the TensorRT version, the
model's SHA-256 and the input shape, so a changed key never reuses an engine. The detector engine
is a setting, CUDA or TensorRT, covered by the `text_detect` fingerprint; CUDA stays the default
until the host bench confirms TensorRT (the numbers are to be measured on the host and are not
recorded here yet). By the owner's choice, the AppImage bundles every TensorRT library the
provider loads: `libnvinfer`, `libnvonnxparser`, `libnvinfer_plugin` and the builder resources,
with ONNX Runtime's `libonnxruntime_providers_tensorrt.so`. `cargo appimage` streams NVIDIA's
7 GB tarball once, checks its pinned size and SHA-256, and keeps only those libraries.

**Consequences:** FP16 comes from TensorRT's own builder, not from a downloaded export. A cached
engine always picks the same kernels, so runs that reuse it give identical results; a rebuild,
after a driver, GPU or TensorRT change, may pick other kernels and give slightly different boxes.
The first run on a new key spends the engine build time, reported apart from screening. The
AppImage grows by the TensorRT libraries (the size is to be recorded on the host), and the first
`cargo appimage` downloads 7 GB more. `libnvonnxparser` and the builder resource are bundled
outside the libraries NVIDIA names as redistributable, for the owner's own use: the AppImage is
not to be shared as is. Outside the AppImage, TensorRT is found only where `cargo appimage`
installed it; Settings does not download it, and a runtime without it still runs the CUDA engine
(`CudaRuntime::tensorrt_available`).

**Supersedes:** none.

### 2026-10-01 — Models may be converted or compiled when a measurement shows it pays

**Context:** Law 4 said models are downloaded already exported (ONNX, GGUF, safetensors) and never
converted, which the foundations entry
[Rust only; FFmpeg as the one external program](/documentation/decisions/foundations.md#2026-09-25--rust-only-ffmpeg-as-the-one-external-program)
set out. TensorRT never runs an ONNX file as it is: its builder compiles each graph into an engine
for one GPU, and FP16 is chosen in that build. No published FP16 export of the PP-OCRv5 mobile
detector was found, so the only FP16 detector is one compiled here.

**Decision:** Models are still downloaded already exported, from pinned URLs with checksums, and
that stays the first choice. A model may be converted, or compiled into an engine such as a
TensorRT engine or an FP16 graph, when a measurement shows it pays. A conversion runs in Rust (law
2) or inside the runtime itself, as TensorRT's builder does; never in Python or another
language's tooling.

**Consequences:** TensorRT engines built from the downloaded FP32 detectors are allowed, cached
per key, and never checked into the repository or pinned as downloads. Each new conversion needs
its measurement first, recorded with the decision that adopts it. CLAUDE.md law 4 and the
inference crate's header say the same.

**Supersedes:** 2026-09-25 — Rust only; FFmpeg as the one external program, for its last
sentence only, that models are never converted by us; the rest of it stands.

### 2026-10-01 — On TensorRT, screening runs at FP16 and confirmation at FP32

**Context:** The detect-bench compared TensorRT FP16 and FP32 with CUDA on Dressrosa 11 and 28.
For screening, FP16 ran at about 250 frames a second against CUDA's 44 and FP32's 113, and on
eleven stretches with real signs its boxes matched CUDA's in number (within a few per stretch) and
in the box images the owner looked at. For confirmation, FP16 found 35 to 70 % fewer boxes than
CUDA on those sign stretches (62 against 121 on Dressrosa 28 at 188 s), while FP32 matched CUDA
within a few boxes (124 against 121) at 9.8 stills a second against CUDA's 7.3.

**Decision:** The pool's options carry the precision per role: `screen_fp16` for the screening and
proxy sessions, `confirm_fp16` for the confirming one. Production builds FP16 screening engines and
an FP32 confirming engine. Whether TensorRT becomes the default engine waits on the determinism
check of the measuring runbook.

**Consequences:** One more engine is cached per frame size and precision. Confirmation keeps
CUDA's recall at a third faster; screening keeps FP16's speed.

**Supersedes:** the single FP16 switch for both roles in the entry of 2026-10-01 — TensorRT runs
the PP-OCRv5 detectors; the rest of that entry holds.

### 2026-10-02 — TensorRT is the default detector engine

**Context:** The measuring runbook's determinism check ran `text_detect` twice on each engine on
Dressrosa 11 on 2026-10-01. The two TensorRT runs on the cached engines gave identical
documents, as did the two CUDA runs. TensorRT took 549 s against CUDA's 1,355 s. Every later
measurement ran on TensorRT, and the owner chose it as the default.

**Decision:** `DetectorEngine::TensorRt` is the default. CUDA remains a setting. Saved settings
that name an engine keep it, and a job keeps the engine it was created with.

**Consequences:**
- Settings without an engine, and new installs, screen on FP16 and confirm on FP32 TensorRT
  engines.
- The first run on a new GPU, driver or TensorRT build spends the engine build time.
- A runtime without TensorRT fails the detection step on the default and needs CUDA chosen in
  Settings.

**Supersedes:**
- "CUDA stays the default until the host bench confirms TensorRT" in the entry of 2026-10-01 —
  TensorRT runs the PP-OCRv5 detectors.
- "Whether TensorRT becomes the default engine waits on the determinism check of the measuring
  runbook" in the entry of 2026-10-01 — On TensorRT, screening runs at FP16 and confirmation at
  FP32.

The rest of those entries holds.
