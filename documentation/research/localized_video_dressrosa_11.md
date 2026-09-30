**Status:** frozen record (2026-09-30)

# The localized video on one episode

The first measurement of in-place replacement, on 2026-09-30: Dressrosa 11 (30.9 minutes,
44,489 frames, 1920×1080 H.264 `yuv420p` at 24 fps, 647 MB) through `tbd-subtitles process` on
the Bazzite host with the RTX 3070 free apart from the desktop. The job resumed its valid audio
and visual steps, so the numbers below are the four replacement steps alone, with the output
step writing the localized subtitle file. They come from the job's `report.md`, its
`steps/*.worker.json`, `visual/text_compose.json` and `visual/localized_video.json`, and ffprobe
on the files beside the video. The steps are described in the
[video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md).

## Runs

| Run | Encoder settings | `localized_video` | Localized video |
|---|---|---|---|
| 1 | `hevc_nvenc -cq 19`, no rate cap | — | 1.16 GB, 1.8 times the source |
| 2 | `hevc_nvenc -cq 19`, peak 1.25 × the source rate | 268.6 s | 707 MB (706,954,737 bytes), 1.09 times the source |
| 3 | as run 2, with the improved stroke separation | 265.3 s | 707 MB (707,001,200 bytes), 1.09 times the source |

The job's probe predated the bit-rate field, so run 2 took the source rate from the file's size
over its duration: about 2.79 Mb/s, a peak of about 3.49 Mb/s. The finished file averages
3.05 Mb/s including its copied AAC audio (the source's video stream alone is 2.62 Mb/s).

## Run 2, step by step

| Step | Wall s | Load s | Peak RAM MiB | Peak child RAM MiB | Peak VRAM MiB | What it did |
|---|---|---|---|---|---|---|
| text_mask | 12.8 | 0.0 | 58 | 262 | — | 21 candidate occurrences; region crops decoded by FFmpeg; 13 fell back |
| text_inpaint | 8.9 | 3.9 | 1,486 | — | 1,202 | LaMa on CUDA over the 28 plates of 8 occurrences |
| text_compose | 0.6 | 0.0 | 32 | — | — | 28 patches lettered in Noto Sans |
| text_typeset | 0.0 | 0.0 | 13 | — | — | the localized events: every occurrence not drawn in, the 13 fallbacks with their warning |
| output | 0.0 | 0.0 | 22 | — | — | `[Muhn Pace] Dressrosa 11.localized.ass` beside the source |
| localized_video | 268.6 | 0.0 | 37 | 514 | 303 | 44,489 frames decoded, blended and encoded with `hevc_nvenc`, 166 frames per second |

The replacement adds 4.9 minutes of wall time to the episode, 4.5 of them encoding; the
decoder and the NVENC encoder, both FFmpeg children, held 514 MiB between them and the encoder
303 MiB of VRAM. Every step stayed far under the 8 GB RAM and 5.5 GB worker VRAM limits.

## Run 3, with the improved stroke separation

Run 3 repeated the replacement steps (`--rerun text_mask`) after the separation learned outlined
lettering on translucent panels, numerals printed on signs and strokes a detector box clips.

| Step | Wall s | Load s | Peak RAM MiB | Peak child RAM MiB | Peak VRAM MiB | What it did |
|---|---|---|---|---|---|---|
| text_mask | 12.8 | 0.0 | — | 262 | — | 21 candidates; 6 fell back |
| text_inpaint | 13.3 | 3.8 | 1,490 | — | 1,202 | LaMa on CUDA over the 67 plates of 15 occurrences |
| text_compose | 0.8 | 0.0 | 33 | — | — | 67 patches lettered in Noto Sans |
| localized_video | 265.3 | 0.0 | — | 515 | 377 | 44,489 frames blended and encoded with `hevc_nvenc` |

## What came out

- 21 occurrences were candidates (displayable English, a keyframe and observed frames).
- Run 2 drew 8 into the video and left 13 as "The writing could not be separated from its
  background", among them the Rebecca role and name cards.
- Run 3 drew 15 into the video:
  - "Sea" six times, for the kanji 海 on a wall in a newspaper photograph the camera pans across
    at 358.5–362 s, five of them grouped as one container;
  - the Rebecca cards at 761.4 and 761.7 s: 「コリーダコロシアム専属剣闘士」 as "Corrida
    Colosseum Exclusive Gladiator" and 「レベッカ」 as "Rebecca", white lettering with a dark
    outline on the card's translucent panel, each pair one container, the furigana erased with
    its line and the panel and the picture behind it kept;
  - the numbers "34" at 795.8 s and "35" three times at 1,056.9–1,059.9 s, printed on signs;
  - "SOL" at 1,620.2 s.
- Run 3 left 6 in the localized subtitle file as "The writing could not be separated from its
  background": the "ワンピース" title logo three times, whose box spans the whole logo artwork;
  two 海 boxes Claude reported that cover only part of the kanji; and a "500" whose box Claude
  placed about 180 pixels from the writing.
- The localized video holds 44,489 frames like the source, one HEVC video stream and the source's
  AAC audio copied, and no subtitle stream; its duration is 1,853.73 s against the source's
  1,853.71 s.
- After run 3 the mask, plate and patch folders took 15, 16 and 22 MB of the work directory.

Whether the replacements read as part of the picture, and whether the file plays in sync in VLC
and mpv, was not judged in this record; both wait for the owner.

## Boundaries

- Depends on: the [in-place replacement decision](/documentation/decisions/stack_and_pipeline.md#2026-09-30--writing-is-replaced-in-a-localized-video-re-encoded-beside-the-source)
  and the [visual scan measurement](/documentation/research/visual_scan_dressrosa_11.md) whose
  steps this run reused.
- Used by: the [roadmap](/documentation/roadmap.md#m5--in-place-on-screen-text) and the
  [feature description](/documentation/features/japanese_onscreen_text.md#replacement-in-the-video).
- Rules: a frozen record keeps its words; a new measurement gets a new record.
