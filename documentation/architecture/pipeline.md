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

## Steps and processes

The job runner (`crates/pipeline/`) runs a stage as one or more
[steps](/documentation/glossary.md#step), in the order of `StepName::ALL`. Each step writes its
own output in the job's work directory, gets its own fingerprint, and gets its own row of time
and peak memory in the job report.

| Step | Stage | Runs in | Output |
|---|---|---|---|
| probe_decode | 1 | worker, `tbd-subtitles` (FFmpeg child) | `probe.json`, `audio/mix_16k.f32` |
| shot_scan | 1 | worker, `tbd-subtitles`, alongside the steps after it | `shots.json` |
| separation | 2 | worker, `tbd-subtitles` (ONNX Runtime) | `audio/vocals_16k.f32`, `audio/background_16k.f32` |
| vad | 3 | job runner | `vad.json` |
| asr_parakeet | 4 | worker, `tbd-subtitles` (ONNX Runtime) | `asr/parakeet.json` |
| asr_whisper | 4 | worker, `tbd-subtitles-ggml` (ggml) | `asr/whisper.json` |
| diff_sheet | 5 | job runner | `sheet.json`, `sheet.txt` |
| sound_events | 8 | worker, `tbd-subtitles` (ONNX Runtime) | `sound_events.json` |
| adjudicate | 6 | worker, `tbd-subtitles` (`claude` children) | `adjudication/first.json` |
| redecode_parakeet | 6 | worker, `tbd-subtitles` (ONNX Runtime) | `adjudication/redecode_parakeet.json` |
| redecode_whisper | 6 | worker, `tbd-subtitles-ggml` (ggml) | `adjudication/redecode_whisper.json` |
| readjudicate | 6 | worker, `tbd-subtitles` (`claude` child) | `adjudicated.json` |
| sound_cues | 6 | worker, `tbd-subtitles` (`claude` children) | `sound_cues.json` |
| alignment | 7 | worker, `tbd-subtitles` (ONNX Runtime) | `aligned.json` |
| cues | 9 | job runner | `cues.json` |
| qc | 10 | job runner | `qc.json` |
| output | 11 | job runner | `<video base name>.srt`, `output.json` |

- **Resume:** a step is skipped when the job record holds its fingerprint and its files exist.
  The fingerprint hashes the step's name and code revision, the settings it reads, the video's
  path, size and modification time (for the steps that read the video), and the fingerprint and
  finish time of every step it reads. A step that runs again therefore re-runs every step after
  it, and `--rerun <step>` forces one.
- **Binaries:** ONNX Runtime, ggml and candle never share a process. `tbd-subtitles` hosts the
  ONNX Runtime, FFmpeg and `claude` workers; `tbd-subtitles-ggml`, built beside it with the
  `crispasr` feature, hosts Whisper.
- **Measurements:** a worker reports its load and processing time, its `VmHWM` and its largest
  child's peak memory. The runner samples its VRAM through NVML. A step in the runner resets and
  reads the runner's own peak memory.

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

- A vocal-separation model on CUDA: Mel-Band RoFormer ONNX (default) or UVR MDX-Net Voc_FT ONNX
  (fast mode). Short-time Fourier transform, chunking and overlap-add are ours (`realfft`).
- Outputs: vocal stem and background stem (mix minus vocals), resampled to 16 kHz mono.
- Speech recognition runs on the original mix by default; the vocal stem feeds voice detection,
  alignment and the vocal half of sound-event detection. The spike measures, per engine, whether
  the vocal stem recognises better than the mix.
- Likely the slowest stage: benchmark first, keep chunks streaming.

## 3. Voice activity and chunk plan

- earshot on the vocal stem: pad 200 ms, merge gaps under 300 ms.
- Chunk plan shared by every engine: 20–60 s chunks cut only at silences of 0.35 s or more, never
  inside speech. Every engine transcribes the same chunks, so hypotheses line up.
- Songs are transcribed like speech here. Every input reads a sung opening as speech. The
  language model flags sung lines `LYRIC` (stage 6), and those lines are neither aligned nor
  shown. The sound-cue step gives each song one music cue.

## 4. Speech recognition

- **Backbone:** NVIDIA Parakeet-TDT-0.6B-v2 (English): fast, no hallucination, word timestamps
  from its token durations.
- **Second engine:** Whisper large-v3 through CrispASR, in the ggml worker binary, over the same
  chunk plan and so only on detected speech; large-v3-turbo is the faster fallback. CrispASR gives
  Whisper no initial prompt, so the glossary reaches only the language model.
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

- Backend: headless `claude -p` (Sonnet) with a JSON schema, eight processes at once; a local
  model through mistral.rs is the offline fallback, available in `inference` but not yet run by
  the app. Input: the diff sheet and the series glossary (names, attacks, places, and alias traps
  such as Lucy vs Luffy; the One Piece glossary is built in and used by default). The model never
  sees or changes timings.
- Output, one JSON object per utterance: `{"id":"U0412","t":"Law, the Birdcage is closing in!","f":[]}`.
  `||` inside `t` marks a speaker change. Flags: `NARR` (narrator, italic), `SPK` (the utterance
  starts with a different speaker than the one before), `LYRIC` (drop), `DROP` (noise), `UNSURE`.
- **Sound cues:** in a step of their own, the model chooses which sound-event candidates become
  cues and words them: lowercase, in brackets, sparse. The candidates are shown window by window,
  with the dialogue around them. A cue keeps its candidate's time. Its text must be one bracketed
  phrase, with capitals only for glossary names. Every song gets a music cue, `[music playing]`
  when the model words none.
- Keeps meaningful interjections and hesitations (`whoa`, `hmm`, `I… I said no!`); drops pure
  filler (`uh`, `um`).
- **Automatic checks, with a re-prompt on failure:** every utterance id returned exactly once;
  every output word appears in some engine's hypothesis of that utterance or a neighbour, or in
  the glossary (otherwise flagged `NOVEL`); no locked non-filler word removed; reading speed above
  25 characters per second flagged.
- **Re-decode:** `UNSURE` utterances are heard again, padded by 0.5 s, on the vocal stem by both
  engines. Their words join the sheet as `ALT p: "…" w: "…"` hypotheses, which count as heard.
  Those ids alone are then adjudicated again, with the settled neighbouring lines as context
  (CrispASR gives Whisper no prompt, so the context reaches the model only). The second answer
  replaces the first for those ids, and the checks run on the merged answer.
- Never rewrites dub lines toward another translation's wording.

## 7. Forced alignment

- Alignment blocks of 20–60 s of the vocal stem, cut at silences of 0.35 s or more; song and
  dropped stretches force block edges.
- Text is converted to spoken form before alignment ("III" → "the third", numbers to words,
  "Señor" → "Senor", hyphens to spaces) with an index map back to the displayed words.
- Aligner: our CTC Viterbi over Parakeet-CTC-0.6B (fp32) frame probabilities, 80 ms frames.
- A block **passes** only if: no run of three or more zero-length or evenly spaced words (the
  signature of a silent aligner failure); every utterance lands within 1 s of its recognition
  window; the median difference from the backbone's word times is 0.2 s or less.
- Fallbacks, in order: align each utterance alone; the backbone's own times (matched word by
  word); for words no source timed, times spread between their timed neighbours. Every word
  records which source timed it (`ctc`, `ctc_utterance`, `backbone`, `interpolated`).
- Offset guard per job: the signed median of aligner start minus backbone start stays under
  30 ms. It is signed because both time words on the same 80 ms grid, where an absolute median is
  one frame; the signed median shows a systematic shift. The quality check reports a larger
  offset.

## 8. Sound events

- CED (AudioSet, 527 classes) over 2 s windows every 0.5 s, on the background stem for effects
  (explosion, crash, gunshot, thunder, shatter, splash, crowd, cheering, applause, footsteps, door,
  knock) and on the vocal stem for non-speech voices (laughter, scream, gasp, grunt, groan, sigh,
  crying, whimper, shout).
- Per-class thresholds, smoothing and minimum durations turn window scores into events.
- **Candidates:**
  - songs are the runs of `LYRIC` lines;
  - effects outside songs;
  - non-speech voices only where nobody is talking, with stricter limits for the classes that
    over-fire (groan, sigh, grunt);
  - Whisper's own sound tags such as `*Grunting*`;
  - each class merged across gaps under 1 s;
  - music under dialogue is never a candidate.
- Candidates go to the sound-cue step (stage 6), which keeps only the ones that matter.

## 9. Cue building

All times snapped to video frames. Full rules: [subtitle style rules](/documentation/architecture/subtitle_style_rules.md).

- One line if the text fits in 42 characters, else two, broken where the grammar allows and
  bottom-heavy; never more than two lines or 84 characters, never over 7 s.
- A speaker's turn splits at sentence ends first, then clauses, then pauses of 250 ms or more.
- In-time: speech onset minus 1 frame. Out-time: last word's end plus 12 frames (0.5 s), then
  extended to at least 20 frames and at most 20 characters per second without cutting text; gaps
  of 3 to 11 frames close to 2 frames.
- Two speakers share a cue (`-Line` per speaker, no space after the hyphen) only when adjudication
  marked the change (`||` or `SPK`), the gap is under 12 frames, and separate cues would break the
  minimum duration or reading speed. Never for the narrator, whose lines are italic. Two short
  lines of one speaker share a cue in the same case. A cue too short for its room may take back
  the lead-out of the cue before, down to that cue's speech and minimum.
- Sound cues take their own cue when there is a gap of 0.8 s or more, else their own line in the
  overlapping cue when it fits, else they are dropped.
- Shot changes:
  - a cut is an scdet change scoring at least the cut score (default 20, `--cut-score`), with
    changes under 0.5 s apart merged into the strongest; threshold 10 over-detects flashes;
  - start on the cut when speech starts within 12 frames after it;
  - end 2 frames before a cut that lies within 12 frames of the end;
  - never straddle a cut by less than 12 frames.
- Priority when rules clash: speech covered, then the 2-frame gap, then minimum duration, then
  shot rules.

## 10. Quality check

Overlaps, gaps under 2 frames, durations, reading speed, line lengths and counts, empty cues, cues
past the end of the video, speech the backbone heard words in for longer than 1 s with no cue
(outside songs and dropped noise; the voice activity with no cue, grunts and crowds included, is
given for reference), unsure lines, novel words, dropped agreed words, utterances timed without the aligner,
the aligner offset and failed model calls, plus the share of words per timing source and of cues
within 20 characters per second. The result is `qc.json`. The runner renders it into `report.md`
after every run, with the flagged lines and their timestamps, the sound cues that found no place,
and one row per step with its time, load, processing, peak RAM, peak child RAM and peak VRAM.

## 11. Output

- `<video base name>.srt`, or `.ass` when positioned sign subtitles exist (M4), UTF-8, in the
  video's folder, written to a part file and renamed. An existing, different subtitle file is
  first copied to the job's `backup/` folder, never beside the video; an identical one is left
  alone.
- The job report stays in the work directory; the GUI shows it.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — processes, crates, work directory.
- [Rust ML stack](/documentation/research/rust_ml_stack.md) — crates and model files per stage.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — layout and timing rules.
- [Speech recognition landscape](/documentation/research/speech_recognition_landscape.md) — why these engines.
