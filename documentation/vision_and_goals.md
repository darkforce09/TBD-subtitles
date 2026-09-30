**Status:** live

# Vision and goals

What TBD-subtitles is for, what it must achieve, what it deliberately leaves out, and how we know
it works.

## Main goal

Give the owner correct, well-timed English subtitles for any video on their disk, with no manual
work: pick a video (or just download it into a watched folder) and a professional-grade subtitle
file appears beside it, which VLC loads automatically.

It exists because the videos the owner watches (fan edits of English-dubbed anime, starting with
the Muhn Pace Dressrosa arc) have no subtitles anywhere, and existing subtitles for the Japanese
version do not match the dub's words or cut.

## Goals

1. **Accurate words.** Transcribe the dub as spoken, with names spelled right (Doflamingo, Trafalgar
   Law, Kyros…). Several speech engines vote; a language model settles disagreements using
   context; nothing is invented.
2. **Precise timing.** Cues start when the speech starts and end when it ends, to the frame, via
   forced alignment of the final text against the audio.
3. **Professional layout.** Netflix English SDH rules: at most two lines of 42 characters, reading
   speed at most 20 characters per second, 5/6 s to 7 s per cue, clean line breaks, speaker
   dashes, italics for narration, shot-change aware timing.
4. **Sound cues (SDH).** Important non-speech sounds in brackets: `[explosion]`, `[laughs]`,
   `[gasps]`. Songs are marked, not transcribed.
5. **Fast and robust.** A 120-minute video in 30 minutes or less end to end on the RTX 3070, in
   bounded memory, resumable after any crash.
6. **Hands-off.** A desktop GUI with a job queue; a right-click "Generate subtitles" entry in the
   file manager; optional watch folders that process new downloads automatically.
7. **Japanese on-screen text.** Signs, letters and title cards written in Japanese get translated
   subtitles, placed near the text (milestone M4).
8. **Simple.** One Rust binary, FFmpeg beside it, models downloaded on first use. No Python, no
   servers, no accounts.

## Non-goals

- No cloud speech APIs by default (researched and declined, see the
  [decisions](/documentation/decisions/)); a cloud backend may be added later as an option.
- No Python, shell or Node anywhere, and no model conversion by us.
- No custom video decoder: FFmpeg does all decoding.
- No transcription of song lyrics; songs get a music cue.
- No full subtitle editor at first: the GUI reviews and fixes flagged lines; deeper editing stays
  with dedicated tools.
- No multi-user, server or mobile version.

## Success criteria

| Area | Measure |
|---|---|
| Accuracy | The owner watches the pilot episode (Dressrosa 11) and accepts it; flagged-unsure lines are few and listed with timestamps |
| Timing | Aligner-to-engine word timing median difference under 30 ms; no speech longer than 1 s left without a cue |
| Layout | QC finds no overlaps, no cue over 42 characters per line or two lines, no cue under 5/6 s; at least 95 % of cues at or under 20 characters per second |
| Speed | 120-minute video in 30 minutes or less; each stage's time recorded in the job report |
| Memory | Peak RAM within 24 GB; each GPU stage fits in 5.5 GB of VRAM |
| Robustness | Killing the app mid-job and restarting resumes from the last finished stage |

## Performance budget

Each stage's budget for a 120-minute video, with the time measured on Dressrosa 11 and scaled to
120 minutes ([stack spike on Dressrosa 11](/documentation/research/stack_spike_dressrosa_11.md#projection-for-a-120-minute-video)):

| Stage (120-minute video) | Budget | Measured, scaled |
|---|---|---|
| FFmpeg decode + shot-change scan (runs alongside the GPU stages) | ≤ 3 min | 1.4 min |
| Vocal separation (Mel-Band RoFormer) | ≤ 12 min | 6.5 min |
| Voice activity detection | < 1 min | 0.0 min |
| Speech recognition, main engine (Parakeet) | ≤ 3 min | 0.5 min |
| Speech recognition, second engine (Whisper large-v3) | ≤ 6 min | 4.8 min |
| Sound events (CED-base, both stems) | ≤ 2 min | 1.1 min |
| Language-model adjudication (`claude -p`) | ≤ 4 min | 2.5 min |
| Forced alignment, cue building, QC | ≤ 2 min | 0.4 min (alignment; cues and QC not built) |
| **Total** | **≤ 30 min** | **about 16 min** |

## Related documentation

- [Roadmap](/documentation/roadmap.md) — the milestones that reach these goals.
- [Pipeline](/documentation/architecture/pipeline.md) — how each goal maps to a stage.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — the layout rules in full.
