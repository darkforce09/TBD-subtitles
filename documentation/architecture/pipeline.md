**Status:** live

# Pipeline

Every stage from a video file to a finished subtitle file: what it reads, what it does, what it
writes, and the guards that keep quality up. The design comes from the Dressrosa research; the
crates and models behind each stage are in the [Rust ML stack](/documentation/research/rust_ml_stack.md),
and the layout rules in the [subtitle style rules](/documentation/architecture/subtitle_style_rules.md).
Stage outputs live in the job's work directory ([system overview](/documentation/architecture/system_overview.md#job-work-directory)).

## Why several engines and a language model

One engine alone is the weak point with anime dubs: music and effects under the dialogue,
shouting, fast exchanges, overlapping lines and invented names. Quality comes from stacking
independent signals, each covering the others' failure modes:

1. Vocal separation, so the engines hear voices rather than the score.
2. Two or three speech engines of different design, whose errors do not coincide.
3. A language model that settles each disagreement with context and a name glossary, under hard
   rules that stop it inventing words.
4. Forced alignment of the final text, which gives the timing.
5. Rule-based cue layout, snapped to frames and shot changes.

## Stage flow

```text
video ─▶ 1 probe+decode ─▶ 2 separate ─▶ 3 vad+chunks ─▶ 4 asr ×N ─▶ 5 diff sheet
                 │                                                        │
                 └─▶ shot changes (parallel, CPU/NVDEC)                    ▼
                                                   6 adjudicate ◀─ 8 sound events
                                                        │ UNSURE → re-decode → 6
                                                        ▼
                             11 output ◀─ 10 qc ◀─ 9 cues ◀─ 7 align
```

## 1. Probe and decode

- ffprobe JSON: streams, languages, durations, frame rate, `start_time`. Pick the English audio
  track by language tag; ask when there are several and none is tagged.
- FFmpeg streams 16 kHz mono f32 (`-f f32le pipe:1`) for recognition and detection, and 44.1 kHz
  stereo for separation, both read in fixed-size chunks through bounded channels; stderr is
  drained on its own thread so the pipe cannot deadlock.
- A second FFmpeg process scans shot changes (`scdet`, or `select=scene` scores on a small scaled
  copy of the video) while the GPU stages run.
- Frame rate from the probe drives all frame snapping (Dressrosa: exactly 24 fps, 41.67 ms per
  frame).

## 2. Vocal separation

- A vocal-separation model on CUDA: UVR MDX-Net Voc_FT ONNX (fast default) or Mel-Band RoFormer
  ONNX (quality mode). Short-time Fourier transform, chunking and overlap-add are ours (`realfft`).
- Outputs: vocal stem and background stem (mix minus vocals), resampled to 16 kHz mono.
- Speech recognition runs on the original mix by default; the vocal stem feeds voice detection,
  alignment and the vocal half of sound-event detection. The spike measures, per engine, whether
  the vocal stem recognises better than the mix.
- Likely the slowest stage: benchmark first, keep chunks streaming.

## 3. Voice activity and chunk plan

- earshot on the vocal stem (Silero for borderline frames): pad 200 ms, merge gaps under 300 ms.
- Chunk plan shared by every engine: 20–60 s chunks cut only at silences of 0.35 s or more, never
  inside speech. Every engine transcribes the same chunks, so hypotheses line up.
- Song stretches (music plus singing from stage 8) are marked so they are not transcribed as
  dialogue and never share an alignment block with speech.

## 4. Speech recognition

- **Backbone:** NVIDIA Parakeet-TDT-0.6B-v2 (English): fast, no hallucination, word timestamps
  from its token durations.
- **Second engine** (third optional), each in its own worker process, chosen in the spike:
  Whisper large-v3 (whisper-rs), Canary or Granite (transcribe-cpp or crispasr), or Kyutai STT 1B
  (candle). Whisper gets the series glossary as its prompt and runs only on detected speech, so it
  cannot hallucinate over music.
- Output per engine: words with start, end and confidence per chunk.

## 5. Diff sheet

- Align each engine's words to the backbone's, chunk by chunk, with a word-level edit-distance
  alignment on normalised words (case, punctuation and number forms folded).
- Split into utterances at pauses and sentence ends. One line per utterance:

  ```text
  U0412 12:03.4 2.1s | Law, the {P:Birdcage|W:bird cage|K:Birdcage} is closing{W:+in}!
  ```

- Words two or more engines agree on are **locked**: the language model may change their case,
  punctuation and glossary spelling only.
- **Orphans:** speech the backbone missed but two or more other engines heard (two words or more,
  inside detected speech) becomes a new utterance.
- Low-confidence agreed words are marked for attention; sound-event candidates are listed with
  their times.

## 6. Adjudication

- Backend: headless `claude -p` with a JSON schema, or a local model through mistral.rs (chosen in
  the spike). Input: the diff sheet, the series glossary (names, attacks, places, and alias traps
  such as Lucy vs Luffy), and optionally reference subtitles as meaning hints. It never sees or
  changes timings.
- Output, one JSON object per utterance: `{"id":"U0412","t":"Law, the Birdcage is closing in!","f":[]}`.
  `||` inside `t` marks a speaker change. Flags: `NARR` (narrator, italic), `LYRIC` (drop),
  `DROP` (noise), `UNSURE`.
- Also chooses which sound-event candidates become cues and words them: lowercase, in brackets,
  sparse.
- Keeps meaningful interjections and hesitations (`whoa`, `hmm`, `I… I said no!`); drops pure
  filler (`uh`, `um`).
- **Automatic checks, with a re-prompt on failure:** every utterance id returned exactly once;
  every output word appears in some engine's hypothesis of that utterance or a neighbour, or in
  the glossary (otherwise flagged `NOVEL`); no locked non-filler word removed; reading speed above
  25 characters per second flagged.
- **Re-decode:** `UNSURE` spans are cut out and re-run with alternatives (mix vs vocal stem, a
  second engine with the neighbouring lines as context), then adjudicated again, those ids only.
- Never rewrites dub lines toward another translation's wording.

## 7. Forced alignment

- Alignment blocks of 20–60 s of the vocal stem, cut at silences of 0.35 s or more; song and
  dropped stretches force block edges.
- Text is converted to spoken form before alignment ("III" → "the third", numbers to words,
  "Señor" → "Senor", hyphens to spaces) with an index map back to the displayed words.
- Aligner: our CTC Viterbi over Parakeet-CTC frame probabilities, or the Qwen3 forced aligner
  where tighter timing is needed (choice from the spike).
- A block **passes** only if: no run of three or more zero-length or evenly spaced words (the
  signature of a silent aligner failure); every utterance lands within 1 s of its recognition
  window; the median difference from the backbone's word times is 0.2 s or less.
- Fallbacks, in order: align each utterance alone; the other aligner; the backbone's own times.
  Every word records which source timed it.
- Offset guard per job: the median aligner-to-backbone difference stays under 30 ms.

## 8. Sound events

- CED (AudioSet, 527 classes) over 2 s windows every 0.5 s, on the background stem for effects
  (explosion, crash, gunshot, thunder, shatter, splash, crowd, cheering, applause, footsteps, door,
  knock) and on the vocal stem for non-speech voices (laughter, scream, gasp, grunt, groan, sigh,
  crying, whimper, shout); CLAP zero-shot for sounds AudioSet lacks.
- Per-class thresholds, smoothing and minimum durations turn window scores into events; music and
  singing regions feed stage 3.
- Candidates go to adjudication, which keeps only the ones that matter.

## 9. Cue building

All times snapped to video frames. Full rules: [subtitle style rules](/documentation/architecture/subtitle_style_rules.md).

- One line if the text fits in 42 characters, else two, broken where the grammar allows and
  bottom-heavy; never more than two lines or 84 characters, never over 7 s.
- A speaker's turn splits at sentence ends first, then clauses, then pauses of 250 ms or more.
- In-time: speech onset minus 1 frame. Out-time: last word's end plus 12 frames (0.5 s), then
  extended to at least 20 frames and at most 20 characters per second without cutting text; gaps
  of 3 to 11 frames close to 2 frames.
- Two speakers share a cue (`-Line` per speaker, no space after the hyphen) only when adjudication
  marked the change, the gap is under 12 frames, and separate cues would break the minimum
  duration or reading speed. Never for the narrator, whose lines are italic.
- Sound cues take their own cue when there is a gap of 0.8 s or more, else their own line in the
  overlapping cue when it fits, else they are dropped.
- Shot changes: start on the cut when speech starts within 12 frames after it; end 2 frames before
  a cut that lies within 12 frames of the end; never straddle a cut by less than 12 frames.
  Priority when rules clash: speech covered, then the 2-frame gap, then minimum duration, then
  shot rules.

## 10. Quality check

Overlaps, durations, reading speed, line lengths, empty cues, cues past the end of the video,
detected speech longer than 1 s with no cue, share of words per timing source, `NOVEL` and
`UNSURE` counts. Written to `report.md` with the flagged lines and their timestamps.

## 11. Output

- `<video base name>.srt`, or `.ass` when positioned sign subtitles exist (M4), UTF-8, in the
  video's folder; an existing subtitle file is backed up first.
- The job report stays in the work directory; the GUI shows it.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — processes, crates, work directory.
- [Rust ML stack](/documentation/research/rust_ml_stack.md) — crates and model files per stage.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — layout and timing rules.
- [Speech recognition landscape](/documentation/research/speech_recognition_landscape.md) — why these engines.
