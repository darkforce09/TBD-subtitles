**Status:** frozen record (2026-09-29)

# The sampled visual scan on one episode

The single-episode measurement of the on-screen text steps, on 2026-09-29: Dressrosa 11
(30.9 minutes, 44,489 frames, 1920×1080 H.264 at 24 fps) through `tbd-subtitles process` on the
Bazzite host with the RTX 3070 free apart from the desktop. The job resumed its valid audio
steps, so every number below is visual work only. The numbers come from the job's `report.md`,
its step logs and its `visual/*.json` documents. The target was the owner's budget of six minutes
of detection for 50,000 frames, that is at least 139 frames per second.

## Runs

| Run | Screening detector | Batch | `text_detect` | Frames per second | Occurrences after detection |
|---|---|---|---|---|---|
| 1 | server PP-OCRv5 | 8 | failed: the 2 GB CUDA arena could not hold a batch of eight | — | — |
| 2 | server PP-OCRv5 | 4 | 494.7 s | 90 | 2,379 |
| 3 | mobile PP-OCRv5 | 4 | 207.8 s | 214 | 237 |
| 4 | mobile PP-OCRv5, centre-and-overlap tracking | 4 | 215.5 s | 206 | 237 |

Run 2 spent its time on the detector: the GPU sat at 88 % while FFmpeg used less than one core.
Its 2,379 occurrences were mostly fragments of the same writing, one animation drawing long,
because the detector's box jitters around static text and the anchor signature was cropped at
each frame's own box. Run 3 compares every picture at the region's fixed anchor box, drops
writing shorter than 0.15 s, wider than half the frame or unconfirmed by the server detector on
its keyframe, and screens with the mobile export; FFmpeg's decoder became the busiest process.

## Run 3, step by step

| Step | Wall s | Peak RAM MiB | Peak VRAM MiB | What it did |
|---|---|---|---|---|
| text_detect | 207.8 | 1,631 | 1,928 | 44,489 proxies screened at two samples per second plus 1,280 shot boundaries; 126 keyframe stills; 237 occurrences |
| text_read | 3.1 | 1,709 | 1,048 | local readings of 237 crops; 131 kept after confident non-Japanese readings (the opening's burned-in English credits) left |
| text_track | 0.0 | 11 | — | sampled geometry check, no decoding |
| text_translate | 30.0 | 3,812 | 3,566 | 56 Claude keyframe requests (54 stills) in parallel, $0.96 in total; Qwen loaded for the two occurrences whose Claude answers failed validation |
| text_review | 0.0 | 19 | — | no corrections |
| text_typeset | 0.0 | 13 | — | 20 rendered occurrences, 40 ASS events |

Visual processing took 4.0 minutes of wall time. Scaled to 50,000 frames, detection takes 3.9
minutes against the six-minute budget. The whole job stayed under the 8 GB RAM and 5.5 GB worker
VRAM limits; the translate worker's 3.8 GB of RAM is the Qwen weights it loaded for two
occurrences.

## What came out

Of the 143 occurrences after translation (131 plus 12 that Claude found beside the listed
regions), 31 carry English, 21 Japanese readings survived, and 20 rendered. The Rebecca role card
「コリーダコロシアム専属剣闘士」 and name card 「レベッカ」 at 761.7–766.3 s, the pilot's annotated
occurrences, were found with frame-exact timing and translated by Claude as "Corrida Colosseum
Exclusive Gladiator" and "Rebecca"; a two-frame fragment of each at the fade-in start stayed a
separate occurrence. Claude's reasons for the unresolved rest name what the screening saw: a
stylized eye, bandage lines, glove shading, Latin lettering on a hat. In run 3 the tracking
step flagged 136 occurrences as moving because its two-pixel corner tolerance did not allow the
detector's box to grow or shrink; run 4, with the centre-and-overlap check, flagged 33, placed 59
nearby and rendered 21 occurrences as 42 ASS events, its translation step taking 10.7 s with 52 of
54 Claude answers served from the cache. No rendered occurrence used a replacement mask: the cards'
backgrounds are not plain enough for the surface check, so English sits beside them.

## Boundaries

- Depends on: the [sampled scan decision](/documentation/decisions/stack_and_pipeline.md#2026-09-29--on-screen-text-is-found-by-sampled-screening-with-bisected-boundaries-and-read-once-per-event-by-claude-vision) and the two entries after it.
- Used by: the [roadmap](/documentation/roadmap.md#m4--japanese-on-screen-text) acceptance and the [feature description](/documentation/features/japanese_onscreen_text.md).
- Rules: a frozen record keeps its words; a new measurement gets a new record.
