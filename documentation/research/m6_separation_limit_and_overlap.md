**Status:** frozen record (2026-10-01)

# Separation within the VRAM cap, and the visual lane beside adjudication

Two M6 changes measured on Dressrosa 11 and 28 against the
[M6 baseline](m6_baseline.md), on the same host and the same day (2026-10-01). The AppImage was
built from commit `e30ead8`, which holds both changes.

- **The VRAM cap** (`f079b28`): every ONNX Runtime CUDA session caps its arena at 4.5 GiB and
  grows it by what each allocation requests. The cuDNN algorithm search keeps its default.
- **The visual lane** (`e30ead8`): `text_detect`, `text_read` and `text_track` run on a thread of
  their own from `adjudicate` until `text_translate` reads them. GPU steps of both walks still
  take the GPU lock one at a time.

## Method

Each run resumed a copy of the episode's baseline job (`--no-library`), so every step before
the first changed one kept the baseline's outputs.

| Run | Episodes | Command | What reran |
|---|---|---|---|
| A | Dressrosa 11 | `process <video> --rerun separation` | every step from separation on; the visual steps read only the probe and the shot scan, so they stayed valid and were skipped |
| B | Dressrosa 11 and 28 | `process <video> --rerun separation --rerun text_detect` | both chains, so the lane ran beside adjudication |

The run A copy of Dressrosa 28 was stopped during separation, once it was clear the lane would
not run.

**Caches.** The copies carried the baseline's reading and translation caches. So in run B,
`text_read` (0.4 s against 6.5 s and 3.5 s) and `text_translate` (14.8 s and 25.3 s against
32.4 s and 44.0 s) mostly served cached answers. Their times are not comparisons.

**Where the numbers come from:**
- each job's `report.md`;
- its step records and `meta/last_run` (`tbd-subtitles dump`);
- `dump outputs` of the deterministic steps, compared with the baseline's;
- SHA-256 hashes of the 16 kHz stems in `audio/`.

## 1. Separation within the VRAM cap

| | Dressrosa 11 baseline | Dressrosa 11 run A / B | Dressrosa 28 baseline | Dressrosa 28 run B |
|---|---|---|---|---|
| Peak VRAM of the worker | 7,264 MiB | 4,174 / 4,174 MiB | 7,266 MiB | 4,174 MiB |
| Wall s | 97.5 | 93.7 / 93.3 | 83.7 | 84.5 |
| GPU busy % | 80 | — / 82 | 80 | 82 |

**Outputs.** The three stems (`vocals`, `background`, `mix`) are byte-identical to the
baseline's on both episodes. The `vad`, `asr_parakeet`, `asr_whisper`, `diff_sheet` and
`sound_events` documents are identical too. The cap moved the worker from 7.3 GB to 4.2 GB at no
cost in time or output, and under the cap the peak no longer depends on how much VRAM is free.
The other ONNX workers stayed as before or shrank:

| Step | Baseline | Run B |
|---|---|---|
| `sound_events` | 1,202 MiB | 850 MiB |
| `text_inpaint` | 1,202 MiB | 954 MiB |
| `alignment` | 3,260 MiB | 3,458 MiB |

The highest worker in run B was `asr_whisper` at 4,412 MiB (ggml, not affected).

## 2. The visual lane beside adjudication

The timeline below is measured from the start of `adjudicate` (step records' finish times less
wall times):

| Run | `text_detect` starts | `adjudicate` ends | `text_detect` ends | redecode starts | `sound_cues` ends | `text_translate` starts |
|---|---|---|---|---|---|---|
| D11 baseline | 245.5 s | 195.8 s | 447.1 s | 195.9 s | 237.4 s | 453.7 s |
| D11 run B | 0.0 s | 138.8 s | 204.9 s | 205.3 s | 251.2 s | 257.5 s |
| D28 baseline | 159.5 s | 101.7 s | 355.3 s | 101.8 s | 152.9 s | 358.9 s |
| D28 run B | 0.0 s | 111.9 s | 198.8 s | 199.5 s | 274.4 s | 281.3 s |

Claude's latency differs between runs (`adjudicate` 195.8 s against 138.8 s, and 101.7 s against
111.9 s). So the saving is measured within each run B: the steps from `adjudicate` up to
`text_translate`, run one after another, would take 396.0 s on Dressrosa 11 and 392.7 s on
Dressrosa 28. With the lane they took 257.5 s and 281.3 s.

| | Dressrosa 11 | Dressrosa 28 |
|---|---|---|
| Saved by the lane | 138.5 s (35 % of that stretch) | 111.4 s (28 %) |
| Real wall time of run B | 758.1 s (12.6 min) | 814.7 s (13.6 min) |
| The same run without the overlap | 896.6 s | 926.1 s |
| Saving on the whole job | 15 % | 12 % |
| Real wall time of the baseline | 981.4 s | 908.2 s |
| Whole-job peak RAM | 2,645 MiB (baseline 3,897 with Qwen) | 2,530 MiB (baseline 2,627) |
| Main walk waiting for the GPU behind `text_detect` | 66.5 s | 87.6 s |

**Peaks.** The highest job RAM was 2,645 and 2,530 MiB, during `adjudicate` with `text_detect`
beside it, against the 24 GB target. During `adjudicate` the job's CPU mean rose from 0.1 to
3.5 cores and its GPU from the desktop's 15 % to 25 %.

**Logs.** The log shows `redecode_parakeet: waiting for the GPU: text_detect of this job is using
it` on both episodes.

**What limits the saving.**
- `text_detect` (205 s and 199 s) outlasts `adjudicate` (139 s and 112 s). The redecode steps,
  which need the GPU, wait for it, and the Claude waits after them (`readjudicate` and
  `sound_cues`, 42 s and 69 s) have nothing beside them.
- Faster detection, where detection is 66 % of `text_detect` with the GPU 28 % busy, would turn
  that wait into saving.

**Outputs.**
- The `text_detect` documents are identical to the baseline's.
- The `text_read` and `text_track` documents equal them within 1e-12. Some confidences differ
  in the last printed digit because run B read them back from the copied reading cache.
- The subtitle files differ from the baseline's: 168 and 208 changed lines in the `.ass`, with
  485 events against 489 and 495 against 491. `text_verify` approved 8 of 8 (baseline 8 of 8)
  and 25 of 31 (baseline 24 of 29).
- These differences come from Claude reruns, not from the lane. Run A, with no lane and only
  Claude rerun after separation, also changed 153 lines of the Dressrosa 11 `.ass`. Adjudication
  marked 6 and 11 lines unsure against 4 and 7.
- Run B of Dressrosa 28 failed the quality check with four stretches of heard speech over 1 s
  with no cue, from that adjudication. The baseline failed with one cue under 20 frames.

## Findings

1. **The VRAM cap holds at no cost.** Separation stays at 4.2 GB, within the 5.5 GB cap, with
   identical stems and the same wall time. Every ONNX worker's peak no longer depends on how much
   VRAM is free.
2. **The lane saves 111–139 s per episode**, 12–15 % of the whole job, within 24 GB.
   - It is limited by `text_detect` outlasting adjudication: the main walk waits 67–88 s for
     the GPU.
   - Faster detection would add up to that much again.
3. **Runs that call Claude again change the subtitles whatever the build.** A comparison of
   subtitle files has to keep the baseline's Claude steps. Items that do not touch the audio
   chain can be compared that way, by rerunning only the steps they change.

## Boundaries

- Depends on: the [M6 baseline](m6_baseline.md), the
  [24 GB decision](/documentation/decisions/foundations.md#2026-09-30--the-pipeline-targets-the-owners-32-gb-machine-24-gb-of-ram)
  and the [pipeline](/documentation/architecture/pipeline.md).
- Used by: the [roadmap](/documentation/roadmap.md#m6--24-gb-workstation-throughput).
- Rules: a frozen record keeps its words; a new measurement gets a new record.
