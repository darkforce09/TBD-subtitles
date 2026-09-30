**Status:** live

# Workstation memory architecture: the 24 GB target

How the pipeline optimizes directly for the owner's 32 GB DDR5-6000 workstation, allocating
a **24 GB RAM target** with a **5.5 GB worker VRAM boundary** to unlock native-resolution visual
screening, 44.1 kHz studio separation, uncompressed video ring buffers, and true audio/visual concurrency.

## Philosophy: target the actual machine

The early milestones established an 8 GB RAM limit to enforce lean, leak-free design. However,
iterating through intermediate 8 GB and 16 GB stepping stones creates throwaway code: downsampling,
aggressive chunking, and disk thrashing methods built for 8 GB are discarded at 24 GB in favor
of fundamentally superior algorithms.

With the owner's machine equipped with **32 GB of DDR5-6000 RAM (approx. 90 GB/s bandwidth)**
and an **i7-14700K (28 threads)**:

1. **Step 1:** Complete the `redb` and `rkyv` binary storage foundation.
2. **Step 2:** Scale directly to the **24 GB target architecture**, bypassing intermediate throwaway steps.
3. **The VRAM boundary stays strict:** Worker VRAM remains capped at **5.5 GB** (respecting the physical
   8 GB limit of the RTX 3070 with display server overhead). GPU stages continue running in separate
   processes under the shared GPU lock.

```text
[Hardware Budget Allocation: 32 GB Host]
├── Host OS, Desktop, Display Server, Browser -> ~8 GB reserved
└── TBD-subtitles Peak Pipeline Target         -> 24 GB RAM / 5.5 GB VRAM
```

---

## 1. Video capabilities at 24 GB

### Full-resolution 1080p visual screening (no 640 proxy)

Downscaling frames to a 640-pixel proxy blurs fine kanji strokes, small background signs, and distant credits.
- **At 24 GB:** `text_detect` screens frames at **native 1080p resolution** utilizing the 28 threads
  of the i7-14700K with batch sizes of 8 to 16.
- Faint, intricate writing and far-off signs are detected with high confidence.

### Zero-stall uncompressed video frame ring buffers (3–4 GB)

In `localized_video`, decoding, compositing, and encoding currently lock-step on tiny pipes.
- **At 24 GB:** an in-memory ring buffer of **3 to 4 GB** holds up to 1,000 raw 1080p frames.
- NVDEC hardware decoding, Rust alpha-blending, and NVENC encoding run completely asynchronously
  at maximum saturation with zero pipe stalling.

### Resident plate, mask, and patch cache (6–8 GB)

[`CACHE_BYTES`](/crates/stages/src/localize/README.md) expands from 512 MB to **6–8 GB**.
- Every single uncompressed RGBA plate, inpaint mask, and tiny-skia patch stays resident in RAM.
- Disk read operations during localized video creation drop to zero.

### Dense optical flow fields across scenes

- Stores dense pixel-by-pixel motion vector fields ($1920 \times 1080$) for entire 10-to-20-second
  shots in RAM.
- English replacement text locks to physical 3D planes with sub-pixel perspective accuracy, eliminating
  wobble and jitter during handheld camera moves.

### Temporal multi-frame inpainting (pixel borrowing)

- Rather than single-image LaMa hallucinating textures, multi-frame video inpainting models inspect
  a 10-to-30-frame window.
- The model borrows real anime background pixels from preceding or future frames where the text was
  not yet visible, producing invisible, artifact-free plate erasure.

---

## 2. Audio capabilities at 24 GB

### Studio-quality 44.1 kHz and 48 kHz separation

Downsampling to 16 kHz cuts off frequencies above 8 kHz, blurring crisp consonant sounds (*s, sh, ch, th, f, t*).
- **At 24 GB:** vocal separation runs at full native **44.1 kHz or 48 kHz stereo**.
- ASR engines hear distinct consonant transients rather than muffled low-frequency approximations,
  eliminating the root cause of phonetic mishears.

### Multi-model separation ensemble (RoFormer + Demucs)

- In loud battle sequences with overlapping brass horns and explosions, an ensemble of
  **Mel-Band RoFormer + HTDemucs v4** processes the audio.
- Blending their spectrogram masks in memory eliminates vocal attenuation and guarantees zero voice dropouts.

### Zero chunk-boundary phase distortion

- Feeds continuous **5-to-10-minute audio windows** into separation networks instead of 10-to-30-second slices.
- Eliminates phase artifacts and volume dips at cross-fade boundaries.

### Global episode acoustic memory and speaker diarization

- Computes an episode-wide **Speaker Acoustic Index**, learning the voice signatures of major characters.
- Muffled dialogue is matched against character acoustic profiles, automating speaker-change tags (`SPK`)
  and disambiguating unclear words.

### Resident spectrogram tensor cache

- The 30-minute episode's Mel spectrogram tensor is computed once into a shared memory block.
- VAD, sound events, alignment, and ASR query the resident tensor with zero redundant CPU computation.

---

## 3. Full pipeline concurrency

- With 24 GB of RAM, the **audio recognition pipeline** (Parakeet, Whisper, Canary) and the
  **visual screening pipeline** (1080p PP-OCRv5) run **simultaneously in parallel**.
- Local translation upgrades to **Qwen 7B or 14B** models.
- Total episode processing wall-clock time drops by **35% to 45%**.

## Boundaries

- Depends on: [`pipeline.md`](/documentation/architecture/pipeline.md),
  [`binary_storage_plan.md`](/documentation/architecture/binary_storage_plan.md), and
  the 32 GB host hardware environment.
- Used by: roadmap milestone M6, `stages::localize`, and worker runners.
- Rules: VRAM remains strictly within 5.5 GB; video source files remain read-only; memory allocations
  are bounded and leak-free.

## Related documentation

- [Roadmap](/documentation/roadmap.md) — milestone M6.
- [Audio accuracy](audio_accuracy.md) — ensembling and vocabulary biasing.
- [Visual and video](visual_and_video.md) — homography tracking and fast re-encoding.
