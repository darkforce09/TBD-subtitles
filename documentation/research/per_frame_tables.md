**Status:** frozen record (2026-10-01)

# Per-frame tables on Dressrosa 11, 28 and a 60 fps copy

What the per-frame `frames` and `readings` tables of the
[binary storage plan](/documentation/architecture/binary_storage_plan.md) (phase 5) cost and
whether they changed any output. Measured on 2026-10-01 on the host (Bazzite, i7-14700K, RTX 3070,
31 GB RAM) with the AppImage built from the phase 5 and 6 code, the sign library empty for the
Dressrosa runs.

## Method

- **Dressrosa 11 and 28.** Each job's documents from before phase 5 up to `text_typeset` were
  stored once by a throwaway seeder outside the repository, then
  `tbd-subtitles process <video> --rerun text_mask` ran the replacement steps, the output and the
  localized video again. The `.ass` and `.localized.ass` files were compared byte for byte with
  those from before phase 3, and each occurrence's `text_verify` verdict with the earlier one.
- **60 fps copy.** FFmpeg made a 60 fps copy of Dressrosa 11 by repeating frames (HEVC NVENC,
  1.2 GB) in a scratch folder; the app ran it from scratch as a new job.
- Step times and peak memory come from each job's `report.md`; rows were counted with
  `tbd-subtitles dump <video> frames` and `readings`.

## Results

| Run | `.ass` / `.localized.ass` | Verdicts | `frames` rows | `readings` rows | `job.redb` |
|---|---|---|---|---|---|
| Dressrosa 11, from `text_mask` | identical | identical (7 occurrences, 7 pass) | 370 | 30 | 4.6 MB |
| Dressrosa 28, from `text_mask` | identical | identical (28 occurrences, 23 pass) | 1,898 | 164 | 18.5 MB |
| Dressrosa 11 at 60 fps, from scratch | — | 8 pass | 1,160 | 39 | 9.2 MB |

| Step | D11 wall s | D28 wall s | 60 fps wall s | Peak RAM MiB (worker) | Peak VRAM MiB |
|---|---|---|---|---|---|
| `text_mask` | 13.8 | 25.4 | 18.9 | 76–84 (child 226–237) | — |
| `text_inpaint` | 10.9 | 52.5 | 12.7 | 1,435–1,477 | 1,202–1,204 |
| `text_compose` | 0.4 | 1.7 | 0.8 | 22–32 | — |
| `text_verify` | 8.1 | 42.1 | 12.2 | 1,455–1,487 | 1,694–2,922 |
| `localized_video` | 265.6 | 228.6 | 661.9 | 21–27 (child 478–486) | 322–396 |

The whole Dressrosa 11 rerun from `text_mask` took 5.0 minutes and Dressrosa 28's 5.9 minutes.
The 60 fps job took 25 minutes from scratch; `text_detect` (317.6 s) and `localized_video`
(661.9 s, 1.12 GB written) grow with the frame count, the per-frame steps stay under 20 s.

## Findings

- Still writing keeps exactly its plates, masks and patches: both episodes' subtitle files and
  verdicts are unchanged, as the design requires.
- The tables are small: at most 1,898 frame rows per episode, and `job.redb` stays under 20 MB.
  A row is a few hundred bytes (the run-length mask dominates), so a two-hour 60 fps video with
  writing on screen throughout stays far below the 432,000-row case of
  [redb_large_transaction_memory](redb_large_transaction_memory.md).
- No replacement step comes near the memory limits: worker RAM stays under 1.5 GB and VRAM under
  3 GB.
- The 60 fps cost is the localized video's encode, proportional to the frame count, not the
  per-frame tables.
