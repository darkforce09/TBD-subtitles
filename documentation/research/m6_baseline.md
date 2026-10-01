**Status:** frozen record (2026-10-01)

# M6 baseline: Dressrosa 11 and 28 from scratch

This record covers where the time, memory, CPU and GPU of a whole job go on the owner's
machine before any throughput item of [milestone M6](/documentation/roadmap.md#m6--24-gb-workstation-throughput)
is built. Every later M6, M7 and M8 target is stated against these numbers.

**How it was measured.** Both episodes were run on 2026-10-01 with
`tbd-subtitles process <video> --work-root <fresh folder> --no-library`, using the AppImage built
from commit `baa1be2`. That commit added the measurements below to the job report. The fresh work
root means empty reading and translation caches, and `--no-library` means no approved sign from
another episode was reused. Every step ran, and none was skipped.

**Where the numbers come from:**
- each job's `report.md`;
- its step records (`tbd-subtitles dump <video> step_records`);
- its `meta/last_run` row.

Copies of these, the subtitle files and the `text_verify` output are kept in
`one_pace/_m6_baseline/d11/` and `d28/`, so later items can be compared against them.

## Host

| Part | State |
|---|---|
| CPU | i7-14700K, 28 threads; governor `performance`, energy preference `performance`, tuned profile `throughput-performance-bazzite` |
| RAM | 31.1 GiB usable, zram swap 15.5 GiB; the desktop used about 6.9 GiB before each run |
| GPU | RTX 3070 8 GB, driver 615.71.09 |
| GPU at idle | over the 30 s before each run (`nvidia-smi`, 500 ms samples), the desktop alone kept the GPU 15–20 % busy and held about 525 MiB |
| Kernel | 7.2.4-ogc3.1.fc44 (Bazzite) |
| FFmpeg | the AppImage's bundled static build (BtbN n8.1.3) |
| Work root | WD Black SN-series NVMe, btrfs (`/var/home`) |
| Source videos | Seagate ST4000DM004 SATA HDD, ext4 (`Main_storage`) |

| Episode | Length | Frames | Video |
|---|---|---|---|
| Dressrosa 11 | 30:53.7 | 44,489 | H.264 1920×1080 yuv420p, 24 fps, 2.79 Mb/s |
| Dressrosa 28 | 26:30.7 | 38,177 | H.264 1920×1080 yuv420p, 24 fps, 3.04 Mb/s |

## What the columns mean

- **× RT**: the video's length divided by the step's wall time. For audio steps this is the
  real-time factor.
- **CPU cores**: how many cores were busy, as the mean and the peak over 250 ms samples. It covers
  the app's whole process tree: the runner, its workers, FFmpeg and `claude`.
- **Hot thread**: the busiest single thread, as its mean share of one core.
- **GPU busy**: NVML's utilisation of the whole device, the desktop's share included. A step
  near 15 % left the GPU idle.
- **Job RAM**: the peak PSS of every process of the job at once.
- **Peak VRAM**: the worker's own allocation as NVML reports it per process; where NVML gives
  none, the device's growth over what was in use before the worker started.

The shot scan runs alongside separation, so the two windows see each other's processes. Steps
under 0.1 s are left out of the tables below. They are `vad`, `diff_sheet`, `review`, `cues`,
`text_track`, `text_review`, `text_typeset`, `qc` and `output`, all under 0.6 s together.

## Dressrosa 11

Real wall time 981 s (16.4 min), against 1,002 s of summed step time. Whole-job peak RAM
3,897 MiB.

| Step | Wall s | × RT | CPU cores mean / peak | Hot thread % | GPU busy % | Peak RAM MiB | Job RAM MiB | Peak VRAM MiB |
|---|---|---|---|---|---|---|---|---|
| probe_decode | 1.0 | 1789 | 2.2 / 3.0 | 72 | 17 | 40 | 73 | — |
| shot_scan | 19.3 | 96 | 14.5 / 21.5 | 94 | 68 | 176 | 1,401 | — |
| separation | 97.5 | 19 | 3.7 / 21.5 | 98 | 80 | 1,259 | 1,401 | **7,264** |
| asr_parakeet | 8.6 | 216 | 3.4 / 4.2 | 93 | 57 | 1,208 | 1,154 | 3,402 |
| asr_whisper | 71.4 | 26 | 1.1 / 1.7 | 97 | 89 | 647 | 598 | 4,412 |
| sound_events | 16.9 | 110 | 1.6 / 19.2 | 99 | 44 | 1,117 | 1,065 | 1,202 |
| adjudicate | 195.8 | 9.5 | 0.1 / 5.8 | 4 | 15 | 268 | 1,208 | — |
| redecode_parakeet | 1.6 | 1171 | 2.5 / 4.3 | 92 | 18 | 1,161 | 1,035 | 3,400 |
| redecode_whisper | 2.1 | 899 | 0.8 / 1.1 | 78 | 74 | 613 | 565 | 3,932 |
| readjudicate | 12.0 | 154 | 0.1 / 0.9 | 5 | 12 | 252 | 279 | — |
| sound_cues | 25.7 | 72 | 0.2 / 3.8 | 6 | 15 | 252 | 938 | — |
| alignment | 8.1 | 230 | 1.7 / 19.5 | 91 | 50 | 1,200 | 1,147 | 3,260 |
| text_detect | 201.5 | 9.2 | 3.0 / 10.1 | 76 | 27 | 1,630 | 2,171 | 1,928 |
| text_read | 6.5 | 284 | 1.5 / 5.6 | 96 | 66 | 1,488 | 1,435 | 1,086 |
| text_translate | 32.4 | 57 | 1.1 / 11.8 | 30 | 17 | 3,944 | 3,897 | 3,566 |
| text_mask | 9.0 | 206 | 1.2 / 2.5 | 73 | 14 | 232 | 298 | — |
| text_inpaint | 12.4 | 150 | 1.5 / 16.2 | 98 | 56 | 1,487 | 1,439 | 1,202 |
| text_compose | 0.8 | 2383 | 0.7 / 0.9 | 71 | 4 | 34 | 65 | — |
| text_verify | 9.7 | 191 | 1.2 / 7.3 | 54 | 9 | 1,497 | 1,619 | 2,894 |
| localized_video | 267.1 | 6.9 | 1.6 / 2.9 | 15 | 6 | 508 | 651 | 275 |

**Peak RAM** is the larger of the step's own peak and its largest child's.

## Dressrosa 28

Real wall time 908 s (15.1 min), against 925 s of summed step time. Whole-job peak RAM
2,627 MiB.

| Step | Wall s | × RT | CPU cores mean / peak | Hot thread % | GPU busy % | Peak RAM MiB | Job RAM MiB | Peak VRAM MiB |
|---|---|---|---|---|---|---|---|---|
| probe_decode | 4.8 | 334 | 0.5 / 0.8 | 15 | 17 | 40 | 73 | — |
| shot_scan | 16.7 | 95 | 14.5 / 23.9 | 96 | 63 | 176 | 1,401 | — |
| separation | 83.7 | 19 | 3.7 / 23.9 | 99 | 80 | 1,259 | 1,401 | **7,266** |
| asr_parakeet | 7.2 | 221 | 3.6 / 4.1 | 96 | 62 | 1,216 | 1,162 | 3,404 |
| asr_whisper | 64.4 | 25 | 1.1 / 1.8 | 98 | 90 | 647 | 597 | 4,412 |
| sound_events | 14.6 | 109 | 1.6 / 19.9 | 99 | 43 | 1,145 | 1,091 | 1,200 |
| adjudicate | 101.7 | 16 | 0.1 / 4.3 | 5 | 15 | 264 | 1,206 | — |
| redecode_parakeet | 1.8 | 903 | 2.4 / 5.0 | 87 | 10 | 1,186 | 1,131 | 3,404 |
| redecode_whisper | 3.1 | 518 | 1.0 / 1.2 | 93 | 84 | 630 | 582 | 4,412 |
| readjudicate | 32.5 | 49 | 0.0 / 0.6 | 3 | 14 | 251 | 279 | — |
| sound_cues | 13.8 | 115 | 0.3 / 4.0 | 8 | 15 | 251 | 941 | — |
| alignment | 6.4 | 250 | 2.0 / 20.0 | 97 | 49 | 1,208 | 1,153 | 3,252 |
| text_detect | 195.9 | 8.1 | 2.8 / 9.6 | 76 | 26 | 1,624 | 2,109 | 1,868 |
| text_read | 3.5 | 454 | 1.8 / 5.3 | 90 | 48 | 1,488 | 1,435 | 1,070 |
| text_translate | 44.0 | 36 | 0.9 / 11.6 | 22 | 28 | 315 | 2,627 | — |
| text_mask | 18.7 | 85 | 1.2 / 4.7 | 49 | 29 | 215 | 310 | — |
| text_inpaint | 42.7 | 37 | 1.2 / 20.0 | 98 | 79 | 1,483 | 1,441 | 1,202 |
| text_compose | 1.7 | 962 | 0.8 / 0.9 | 76 | 18 | 26 | 65 | — |
| text_verify | 36.5 | 44 | 0.9 / 8.3 | 52 | 12 | 1,499 | 1,611 | 1,706 |
| localized_video | 230.3 | 6.9 | 1.6 / 2.5 | 15 | 8 | 509 | 652 | 275 |

## Phases

| Phase | Dressrosa 11 | Dressrosa 28 |
|---|---|---|
| `text_detect` decode wait (360-line proxy, CPU) | 14.1 s (7 %) | 11.9 s (6 %) |
| `text_detect` detection (mobile PP-OCRv5, batch 4) | 132.7 s (66 %) | 127.8 s (65 %) |
| `text_detect` confirmation (server detector on keyframes) | 26.7 s (13 %) | 27.0 s (14 %) |
| `text_detect` full-resolution stills | 19.8 s (10 %) | 20.6 s (11 %) |
| `text_detect` frames decoded / screened | 44,489 at 221 fps / 11,694 at 58 fps | 38,177 at 195 fps / 11,096 at 57 fps |
| `localized_video` decode wait (CPU) | 28.5 s (11 %) | 25.2 s (11 %) |
| `localized_video` blend | 0.6 s (0 %) | 1.0 s (0 %) |
| `localized_video` encode wait (`hevc_nvenc -preset p6`) | 236.8 s (89 %) | 202.9 s (88 %) |
| `localized_video` flush | 0.3 s | 0.3 s |
| `localized_video` rate, NVENC use | 166.6 fps, NVENC 100 % | 165.8 fps, NVENC 99 % |
| `shot_scan` rate | 2,302 fps | 2,289 fps |
| `text_verify` samples | 38 at 4.2 per second | 142 at 4.0 per second |

## Model calls and output

| | Dressrosa 11 | Dressrosa 28 |
|---|---|---|
| Claude calls, adjudicate / readjudicate | 8 / 9 ($1.23 / $1.25) | 8 / 9 ($0.89 / $0.93) |
| Sound cues | 43 candidates, 27 cues, $0.17 | 36 candidates, 16 cues, $0.13 |
| Local Qwen3.5-4B loaded in `text_translate` | yes (3.9 GB RAM, 3.6 GB VRAM) | no |
| Cues in `.ass` | 471 (445 dialogue, 22 sound, 4 music) | 411 |
| Occurrences detected, after reading / translated | 237, 130 / 18 | 152, 73 / 49 |
| Replacements baked / approved by `text_verify` | 8 / 8 (38 samples) | 29 / 24, 5 left with Japanese (142 samples) |
| Localized video | 692 MB | 632 MB |
| Quality check | one cue under 20 frames ("Again?", 15 frames) | one cue under 20 frames |

**From-scratch runs do not repeat.** Each run's subtitle files differ from the previous outputs of
the same episodes (moved to `_m6_baseline/previous/`). Dressrosa 11 has 489 events in its `.ass`
against 467, and Dressrosa 28 has 24 approved replacements against 25. Adjudication and
translation are Claude answers, and these runs had no sign library.

So an M6 item is compared by resuming a copy of the baseline job from the first step it changes.
The Claude answers and caches of the steps before that step are then reused, and identical
subtitle files and `text_verify` verdicts can be asked of it. A second from-scratch run is not a
valid comparison.

## Findings

1. **Separation breaks the VRAM cap.**
   - The separation worker held 7,264 and 7,266 MiB: almost all of the 7,360 MiB free when it
     started, against the 5.5 GB cap.
   - Its ONNX Runtime session uses the default CUDA provider: no memory limit, an arena that
     grows by powers of two, and an exhaustive cuDNN algorithm search with a workspace sized to
     the free memory.
   - Earlier reports show about 4,280 MiB, when less memory was free.
   - The PP-OCRv5 sessions already set a 3 GB limit, `SameAsRequested` growth and a heuristic
     search. Separation sets none of them.
2. **Three steps take two thirds of the time:**

   | Step | Dressrosa 11 | Dressrosa 28 |
   |---|---|---|
   | `localized_video` | 27 % | 25 % |
   | `text_detect` | 21 % | 22 % |
   | `adjudicate` | 20 % | 11 % |

3. **`localized_video` is limited by the encoder.**
   - The single NVENC engine was busy 99–100 % at 166 fps, and 89 % of the step was spent
     waiting on the encoder pipe.
   - Decoding waited 11 % and the blend under 1 %. Of the step's time, 28 s and 25 s are decode
     and blend that a queue could hide behind the encoder.
   - NVDEC would only make that hidden part smaller.
4. **`text_detect` is limited neither by the GPU nor by decoding.**
   - Detection takes 66 % of the step at 58 screened frames per second, yet the GPU was 26–27 %
     busy (desktop included). One thread ran at 76 % and the mean was 3 cores.
   - The proxy decoder kept up: decode wait was 7 %.
   - So the time goes to work around the detector calls: preprocessing, small batches of four,
     synchronous calls and postprocessing. The GPU has headroom for the six-fold pixels of
     full-resolution screening once that work runs in parallel. Decoding at full resolution
     moves six times the bytes per frame through the pipe, which is where NVDEC and scaling on
     the GPU would count.
5. **Steps that wait on Claude leave the machine idle.**
   - `adjudicate`, `readjudicate`, `sound_cues` and most of `text_translate` add up to 266 s on
     Dressrosa 11 and 192 s on Dressrosa 28. During them the GPU stays at the desktop's idle level
     and the CPU at 0.1–1 core.
   - Adjudication alone varies from 102 to 196 s with Claude's latency.
   - `text_detect`, `text_read` and `text_track` need only the probe and the shot scan. They take
     208 s and 199 s, about as long as the Claude-bound audio steps.
6. **The rest:**
   - `separation` (80 % GPU) and `asr_whisper` (89–90 % GPU) are limited by the GPU, at 19× and
     25× real time.
   - The CPU is mostly idle: no step averages more than 3.7 cores except the shot scan (14.5).
     Every GPU worker runs one hot thread at 90–99 %.
   - Whole-job RAM peaks at 3.9 GB, with the Qwen weights loaded, against the 24 GB target.
7. **The local translation model** loaded on Dressrosa 11 only, where Claude left translations
   unanswered. `text_translate` took 32 s there, its 3.9 GB of RAM being the job's peak. It is a
   matter of quality, not of time.

## What this says for M6

The remaining items are ranked by what they can remove from the real wall time measured above:

| Item | Bottleneck evidence | Time it can remove, D11 / D28 |
|---|---|---|
| Separation VRAM limit (law 5) | 7.26 GB against a 5.5 GB cap | none; required, its time measured |
| Overlap `text_detect`/`read`/`track` with the Claude-bound audio steps | 266 / 192 s of Claude waiting; 208 / 199 s of visual prefix that needs neither | about 200 / 190 s (20 %) |
| Faster detection (batching, parallel pre- and postprocessing), then full-resolution screening | detection 66 % of `text_detect` with the GPU 27 % busy | the proxy time first; full resolution then costs what the GPU's headroom does not absorb |
| NVDEC for the screen | decode wait 7 % at the proxy | small at the proxy; needed for full resolution |
| Bounded frame queues in `localized_video` | decode and blend 11 % of the step, with NVENC already at 100 % | at most 29 / 26 s (3 %) |
| NVDEC for the localized video | decode is hidden once queues exist | almost none |
| 7B local translation model | quality only; loads only for what Claude leaves | none; a quality trial |

The encoder bound is outside the M6 list:
- re-encoding only what changed (M8);
- a faster NVENC preset than `p6`, which would change the video's bytes and size.

## Boundaries

- Depends on: the [24 GB decision](/documentation/decisions/foundations.md#2026-09-30--the-pipeline-targets-the-owners-32-gb-machine-24-gb-of-ram),
  the measurements of the [pipeline](/documentation/architecture/pipeline.md) report, and the
  [memory profiles](/documentation/optimizations/memory_profiles.md).
- Used by: the [roadmap](/documentation/roadmap.md#m6--24-gb-workstation-throughput) and every
  later M6–M8 measurement.
- Rules: a frozen record keeps its words; a new measurement gets a new record.
