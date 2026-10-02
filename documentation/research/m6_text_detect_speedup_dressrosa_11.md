**Status:** frozen record (2026-10-02)

# The text_detect speed-up on Dressrosa 11

The M6 work that took `text_detect` on Dressrosa 11 (44,489 frames, 30:54) from 549 s to about two
minutes on TensorRT, and the review that followed it, on the same host on 2026-10-01 and
2026-10-02:

- commits `e163fe9` to `2ec84f0` on `main`;
- the review's fixes: the keyframe candidates of noise released, entries carried across
  screening groups, TensorRT as the default engine with one confirming session on CUDA, and
  revision 6.

## Method

**The commit series.** Each commit's message records a `--rerun text_detect` of the owner's
Dressrosa 11 job on TensorRT. Section 1 quotes those figures; the runs themselves were not kept.

**The owner's job.** `text_detect` at `2ec84f0`, from the job's `report.md` of 2026-10-02 18:29;
the job of 2026-10-01 19:33 on TensorRT is the comparison.

**The review pilots.** `visual_validation run <Dressrosa 11> <work> --binaries <bin>` on the host
runs the six visual steps of a fresh work folder with production workers, on the default engine
and without Claude. It was stopped once `text_detect` was stored. Documents and step records come
from `tbd-subtitles dump <work> outputs|step_records text_detect`, and the PNGs were compared
with `cmp`.

| Build | Code |
|---|---|
| A | `2ec84f0` with TensorRT as the default and the stdin feeder then uncommitted in `child_process` |
| B1 | A, plus the candidates of noise released, the main binary's workers refusing a terminal, and the feeder joined only after a timeout |
| B2 | B1, plus entries carried across screening groups and revision 6 |

Runs A and B1 waited 0.1 s on the decoder. While A2, B1b and B2 ran, another program compiled
on the host (load average about 13). Their decode wait rose from 0.1 s to 25–35 s, so their wall
times are not comparisons; their counts are.

## 1. The commit series

| Commit | Change | Dressrosa 11, from the commit message |
|---|---|---|
| `e163fe9` | full-resolution screening at the 0.3 box score, the 640-wide pass, TensorRT FP16 screening and FP32 confirming | 548.9 s, 725 occurrences (the job of 2026-10-01) |
| `9da9bf2` | screening at 0.45, regions under 12 px dropped, probes without the 640-wide pass | — |
| `88841aa` | the OCR workers' arena limit from 3 to 5 GiB, `text_verify`'s VRAM need from 2,894 to 3,800 MiB | — |
| `06f0d6a` | keyframe candidates within 16 GiB instead of 4 | 75.5 s of FFmpeg seeks gone |
| `3b42426` | bisection only once writing has persisted | 471.8 → 248.4 s |
| `6f57fd6` | confirmation only for five frames or more and a score of at least 0.50 | confirmation 99.9 → 26.6 s |
| `e8a0b24` | confirmation at 0.55, flat regions dropped, the signature loops pruned | confirmation 22.0 s, 5,535 tracks fewer |
| `896884e` | bisection after three samples or a cut; pending entries dropped when a group closes | probe frames 3,850 → 2,011, probes 64.4 → 43.6 s, the step 148.5 → 119.8 s |
| `2ec84f0` | a 45-second decode queue, six waiting groups per session, signature matches kept per group, two confirming sessions on TensorRT | the owner's job: 124.3 s |

## 2. The owner's job at `2ec84f0`

| | 2026-10-01, `e163fe9` | 2026-10-02, `2ec84f0` |
|---|---|---|
| `text_detect` wall | 548.9 s | 124.3 s |
| `text_detect` peak RAM | 7,267 MiB | 17,481 MiB |
| `text_detect` peak VRAM | 1,894 MiB | 3,222 MiB |
| Occurrences detected | 725 | 103 |
| Translated | 71 | 15 |
| Replaced in the localized video / left in Japanese | 27 / 27 | 3 / 7 |
| Visual processing | 10.5 min | 2.5 min |

The phases of the 124.3 s were:

| Phase | Time | Share |
|---|---|---|
| decode wait | 35.1 s | 29 % |
| conversion | 7.2 s | 6 % |
| probes | 31.2 s | 26 % |
| signatures | 9.9 s | 8 % |
| confirmation | 18.7 s | 15 % |

44,489 frames were decoded (364 a second) and 6,741 screened. `text_verify` peaked at 1,948 MiB
of VRAM.

The subtitle files of the two days hold these English signs (2026-10-01: either of the TensorRT
and CUDA runs; 2026-10-02: the job's latest file):

| Sign | 2026-10-01 | 2026-10-02 |
|---|---|---|
| "One Piece" card, 0:21, 1.4 s | yes | no |
| 正義 ("Justice"), 1:21, 0.25 s flashes | yes | no |
| 海 on a wall ("Sea"), 5:58, 4 s | yes | no |
| "Dead or alive", 6:09 | yes | yes |
| Colosseum lunch signs, 11:35–11:42 | yes | yes |
| Gladiator card and "Rebecca", 12:39 | yes | yes |
| Number boards 33–36, 12:18 and 15:16, up to 1.3 s | yes | one (34) |
| "SOL" | yes | yes |

Claude's wording, and whether it renders a sign at all, varies between runs. But the pilots below
find no occurrence over the 海 wall (358.5–362.5 s), so it is lost at detection.

## 3. The review's pilots

| | A | A2 | B1 | B1b | B2 |
|---|---|---|---|---|---|
| `text_detect` wall | 97.6 s | 143.8 s | 103.5 s | 138.7 s | 150.8 s |
| Peak RAM | 19,471 MiB | 17,801 MiB | 9,875 MiB | 9,172 MiB | 8,631 MiB |
| Peak VRAM | 3,222 MiB | 3,222 MiB | 3,222 MiB | 3,222 MiB | 3,222 MiB |
| Keyframe candidates at peak | 14,339 MiB | 14,339 MiB | 3,070 MiB | 3,070 MiB | 3,087 MiB |
| Frames probed | 1,416 | 1,416 | 1,416 | 1,416 | 1,950 |
| Frames screened | 6,741 | 6,741 | 6,741 | 6,741 | 7,275 |
| Keyframes confirmed (from RAM / FFmpeg) | 179 / 0 | 179 / 0 | 179 / 0 | 179 / 0 | 196 / 0 |
| Occurrences | 131 | 131 | 131 | 131 | 148 |
| Probes | 34.0 s | 32.4 s | 38.1 s | 36.2 s | 47.0 s |
| Confirmation | 18.7 s | 18.3 s | 19.0 s | 18.3 s | 19.9 s |
| Decode wait | 0.1 s | 32.5 s | 0.1 s | 35.2 s | 25.6 s |

- **Determinism and the release of noise.** The documents of A, A2, B1 and B1b are byte-identical
  (SHA-256 `f0338c2c…`), and so are their 131 crops and 55 keyframe images. Releasing the
  candidates of writing that ends without persisting changed nothing but memory: the candidates
  fell from 14,339 to 3,070 MiB and the step from 19,471 to 9,875 MiB. A had 14,339 MiB
  at its peak, under the 16 GiB budget less one frame, so nothing was evicted. In that case a
  smaller held set cannot change the document.
- **Entries across groups.** Against B1, B2 adds a bisected entry frame to 12 occurrences. They
  start 0.04–0.46 s earlier, median 0.21 s, and the keyframes of 4 of them move one sample
  earlier with their middle. B2 also keeps 17 more occurrences: 9 reach five frames with their
  entry and qualify for confirmation, and 8 are confirmed on a keyframe they share with writing
  that qualifies. Two of those 8 share it with writing the server detector then rejected. Among
  the 17 are the "SOL" sign at 1,458 s and a "Colosseum" banner at 698 s, beside cursive scribbles
  at 357 s. No occurrence is lost; the other 119 are byte-identical. The cost is 534 more probe
  frames and 17 more confirmations, about 10 s more of probing.
- **Wall time.** A and B1, on a quiet host, took 97.6 s and 103.5 s. The loaded runs say nothing
  about the code.

## Conclusions

- On Dressrosa 11 on TensorRT, `text_detect` takes about 100 s on a quiet host and peaks under
  10 GiB of RAM with the release of noise's candidates. Before the release it peaked near 19 GiB,
  most of it keyframe candidates of writing that had already ended.
- Most of the speed-up comes from not bisecting and not confirming screening noise. The cost is
  writing shown for less than about two seconds, which the owner accepts, and the 海 wall, which
  is open.
- The worst case of the scan's memory bounds sums the queue, the full candidate budget and the
  waiting groups. That is close to 23 GiB (24.7 GB), above the job's 24 GB, while the measured candidates
  stay near 3 GiB.
