**Status:** frozen record (2026-09-26)

# Pilot run on Dressrosa 11

The first end-to-end run of the pipeline, `tbd-subtitles process`, on
`[Muhn Pace] Dressrosa 11.mp4`, on 2026-09-26. The episode is 30.9 minutes long (1853.7 s, 24 fps).
It ran on the Bazzite host, with the RTX 3070 free apart from the desktop. Every number comes from
the job's `report.md` and its step outputs. The episode had no reference transcript, so quality is
judged by the quality check here and by the owner's viewing in VLC. The decisions this run led to
are in the [decision log](/documentation/decisions/), dated 2026-09-26.

## 1. Steps, time and memory

| Step | Wall s | Load s | Peak RAM MiB | Peak child RAM MiB | Peak VRAM MiB |
|---|---|---|---|---|---|
| probe_decode | 4.8 | 0.0 | 8 | 75 (FFmpeg) | — |
| shot_scan (alongside) | 21.0 | 0.0 | 8 | 211 (FFmpeg) | — |
| separation (Mel-Band RoFormer) | 103.6 | 1.3 | 1156 | 271 (FFmpeg) | 4282 |
| vad | 0.6 | — | 15 | — | — |
| asr_parakeet | 8.7 | 1.7 | 1205 | — | 3406 |
| asr_whisper (large-v3) | 74.7 | 1.5 | 630 | — | 4412 |
| diff_sheet | 0.0 | — | 16 | — | — |
| sound_events (CED-base) | 17.6 | 0.3 | 1127 | — | 1202 |
| adjudicate (`claude -p`, 8 at once) | 52.9 | — | 11 | 267 (`claude`) | — |
| redecode_parakeet (4 utterances) | 2.9 | 2.3 | 1181 | — | 3406 |
| redecode_whisper (4 utterances) | 3.5 | 2.0 | 604 | — | 4412 |
| readjudicate | 8.9 | — | 10 | 252 (`claude`) | — |
| sound_cues | 5.6 | — | 11 | 251 (`claude`) | — |
| alignment (CTC Viterbi) | 7.8 | 0.9 | 1197 | — | 3252 |
| cues, qc, output | 0.0 each | — | 16 | — | — |

**The budget:**
- The steps add up to 5.2 minutes. Scaled to a 120-minute video, that is 20.2 minutes against
  the 30-minute budget. The scaling counts the shot scan, which runs alongside, and every load
  time, so it is an upper bound.
- Every GPU step stayed under the 5.5 GB VRAM budget; Whisper came closest at 4412 MiB.
- The largest process peaked at 1205 MiB of RAM, far under the 8 GB budget.
- The times match the stack spike's.
- The language-model steps cost $0.72 at list price, against the owner's subscription.

## 2. What the first run found, and what changed

The first complete run (468 cues) passed the reading-speed target, with 97.2 % of cues at or
under 20 characters per second. It broke three rules and gave one misleading figure:

- **11 cues under 20 frames.** Quick exchanges spread over separate utterances, such as "What is
  his deal?" followed 0.6 s later by "I don't know.", left no room for a cue of its own. The model
  had marked only 2 speaker changes in the whole episode, all inside utterances, so no
  two-speaker cue was allowed.
  - **Change:** the `SPK` flag. A cramped cue now shares a cue with its neighbour: dashed when
    the speaker changes, plain when the speaker stays. A cramped cue may take back the previous
    cue's lead-out.
  - **Result:** the model flagged 194 utterances `SPK`; 8 two-speaker cues appeared and no cue is
    under 20 frames.
- **Three music cues for one opening.** The lyric runs of the opening song were split wherever a
  gap of 5 s or more fell between lyric lines.
  - **Change:** runs join across instrumental breaks under 30 s that hold no spoken line.
  - **Result:** the opening got one music cue, then its spoken lines ("My darkness will swallow
    the world…"), then a second music cue after them.
- **215 s of "speech with no cue".** Neither engine heard a word in these stretches: they were
  voice activity from grunts, crowds and shouts. A second attempt counted words either engine
  heard, and still flagged 73 stretches. Most were Whisper's laughs ("ha ha ha"), which the model
  dropped, and Whisper word ends stretched over the silence after.
  - **Change:** the check counts only the backbone's words, each counted for at most 1 s.
  - **Result:** no heard speech is left without a cue. The 215 s of voice without a cue is
    reported beside it.
- **The `audio/` folder** was not created before the mix was streamed into it. The first attempt
  stopped at the first step with that error.

## 3. The finished file

The finished file is `[Muhn Pace] Dressrosa 11.srt`, beside the video.

**Cues:**
- 448 cues: 432 dialogue, 12 sound, 4 music.
- 99.1 % of cues are at or under 20 characters per second.
- No layout rule is broken: no overlap, no gap under 2 frames, no cue under 20 frames or over
  7 s, no line over 42 characters, no third line.

**Sound cues:**
- The model chose 17 of 43 candidates.
- It worded them as `[explosion]`, `[gunshot]`, `[screams]`, `[sighs]`, `[laughs]`,
  `[coughing]`, `[whimpering]` and `[grunting]`, and the songs as `[theme song playing]`,
  `[upbeat music playing]` and `[gentle music playing]`.
- One sound went in as a line of a dialogue cue, and none was dropped.

**Adjudication:**
- The first pass flagged 4 utterances `UNSURE`. After both engines heard them again on the vocal
  stem, the second pass settled all 4.
- 32 lines were flagged as lyrics.

**Timing:**
- Words were timed by the CTC aligner over blocks (2436), by the aligner over single utterances
  (240), by the backbone (8), and by interpolation (14).
- The aligner's signed median offset from the backbone was +20 ms.

**Left for review** (7 findings in `report.md`):
- 4 cues over 20 characters per second (20.4 to 23.1): fast lines with no room to extend.
- 2 "novel" words: the `D` of "Block D", and "Zahahahahaha!" (Blackbeard's laugh, spelled from
  "zahaha"). Both are faithful to the dub.
- 1 utterance of 22 words at 12:01 timed by the backbone, because its alignment failed the checks.

## 4. Resume

- **Killed mid-step:** the app was killed with SIGTERM six seconds into a re-run of
  `sound_events`. Its worker died with it (`PR_SET_PDEATHSIG`); no process of the app was left
  and the GPU held only the desktop's. The next run took over the stale lock, skipped the 12
  valid steps, and redid `sound_events` and the steps after it that read it: `sound_cues`,
  `cues`, `qc` and `output`.
- **Deleted output:** with `cues.json` deleted, the next run redid only `cues`, `qc` and `output`.

## Hard gaps

- **Speaker changes come from the text alone:** no diarisation runs, so a wrong `SPK` guess
  shows as a wrong dash or a missing one.
- **Fast lines:** 4 cues stay over 20 characters per second. Text is never cut, and the cues
  around them leave no room.
- **Timing of the backbone fallback:** an utterance whose alignment fails keeps Parakeet's
  80 ms word times, which are good but not frame-exact.
- **Owner's viewing pending:** only the owner's viewing in VLC can judge the words, the sound
  cues and the feel of the timing.

## Sources

[Stack spike on Dressrosa 11](/documentation/research/stack_spike_dressrosa_11.md) ·
[Pipeline](/documentation/architecture/pipeline.md) ·
[Subtitle style rules](/documentation/architecture/subtitle_style_rules.md)
