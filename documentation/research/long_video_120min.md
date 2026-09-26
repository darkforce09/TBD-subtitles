**Status:** frozen record (2026-09-26)

# A 120-minute video through the pipeline

The speed and memory test of the pipeline on a video longer than two hours, on 2026-09-26. No
episode in the media folder is that long (the longest, Dressrosa 39, runs 43.6 minutes), so
Dressrosa 39, 48 and 40 were joined, by stream copy with FFmpeg's concat demuxer, into one file of
128.9 minutes (7732.3 s, 24 fps, H.264 and AAC) under `~/.local/share/tbd-subtitles/test-media/`.
The episodes were only read. The job ran with `tbd-subtitles process` at commit d70e9d8 on the
Bazzite host, the RTX 3070 free apart from the desktop. Every number comes from the job's
`report.md`; the budget is the [performance budget](/documentation/vision_and_goals.md#performance-budget).

## 1. Time and memory

| Step | Wall s | Peak RAM MiB | Peak child RAM MiB | Peak VRAM MiB |
|---|---|---|---|---|
| probe_decode | 3.5 | 8 | 97 (FFmpeg) | — |
| shot_scan (alongside) | 85.7 | 8 | 240 (FFmpeg) | — |
| separation (Mel-Band RoFormer) | 415.4 | 1159 | 272 (FFmpeg) | 4280 |
| vad | 2.3 | 16 | — | — |
| asr_parakeet | 30.1 | 1259 | — | 4434 |
| asr_whisper (large-v3) | 304.4 | 635 | — | 4412 |
| sound_events | 69.5 | 1157 | — | 1200 |
| adjudicate | 172.1 | 14 | 310 (`claude`) | — |
| redecode_parakeet | 3.8 | 1190 | — | 3406 |
| redecode_whisper | 18.7 | 618 | — | 4412 |
| readjudicate | 89.6 | 16 | 268 (`claude`) | — |
| sound_cues | 17.7 | 16 | 251 (`claude`) | — |
| alignment | 26.1 | 1209 | — | 3260 |
| diff_sheet, cues, qc, output | 0.0 each | 23 | — | — |

**The budget:**
- The whole run took 1154 s of wall time: 19.2 minutes for 128.9 minutes of video, 17.9 minutes
  per 120 minutes, against the 30-minute budget. The report's sum of step times, which counts the
  shot scan running alongside, is 20.6 minutes, 19.2 per 120 minutes.
- The largest process peaked at 1259 MiB of RAM; audio stayed streamed.
- Every GPU step stayed under the 5.5 GB VRAM budget; Parakeet came closest at 4434 MiB.
- A debug build of the workspace ran on the same CPU for about a minute during separation.

## 2. Quality check

1713 cues (1658 dialogue, 51 sound, 4 music); 99.7 % at or under 20 characters per second; no
heard speech without a cue; aligner offset +20 ms. 55 findings: 11 unsure, 12 novel words, 17
dropped agreed words (mostly spelling fixes such as Zorro to Zoro), 7 lines timed without the
aligner, 5 fast cues, and **3 cues under 20 frames**, which break a layout rule: the
interjections "ha!", "Ah!" and "Huh?", each between two cues too full to share one with.

## Hard gaps

- **Short interjections in dense exchanges:** the cue stage leaves a one-word cue under 20 frames
  when both neighbours are full; such a job fails the quality check.

## Sources

[Pilot run on Dressrosa 11](/documentation/research/pilot_dressrosa_11.md) ·
[Pipeline](/documentation/architecture/pipeline.md)
