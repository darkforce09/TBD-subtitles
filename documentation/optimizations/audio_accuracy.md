**Status:** live

# Dialogue accuracy: missed speech, names and separation

How the dialogue could miss fewer lines and spell names right more often. These are the items of
milestone M7; each is built only when the owner picks it, and each is measured on real episodes
against the baseline of [memory profiles](memory_profiles.md#1-baseline-first). No item may let a
subtitle word come from anywhere but a speech engine (law 8).

## What the finished jobs show

The work directories of Dressrosa 11–48 show where dialogue goes wrong today:

- **Speech in a chunk the backbone left empty is lost.** `diff_sheet` cuts utterances from the
  backbone engine's (Parakeet's) words. In
  [`sheet.rs`](/crates/stages/src/diff_sheet/sheet.rs), `cut_points` returns no cut for a chunk
  with no backbone word, so the loop over its pieces never runs, and whatever Whisper heard in that
  chunk never reaches the sheet. The quality check cannot see it either: its "heard speech with
  no cue" counts Parakeet's words only. How often this happens is not yet counted.
- **Names.** The diff sheets hold 36 places where Parakeet heard "Flamingo" and Whisper
  "Doflamingo", and 5 the other way round. Adjudication, with the glossary, settles nearly all
  of them; three lines of the finished subtitle files of Dressrosa 12–48 say "Flamingo", and
  whether the dub says so there is not checked.
- **Unsure lines are few.** After the re-decode, 24 of the 39 job reports have no line still
  unsure, and the most is 10.

## 1. Orphan recovery in `diff_sheet`

- **What changes:** in a chunk where the backbone has no word but another engine heard words, those
  words become an [orphan](/documentation/glossary.md#orphan) utterance on the sheet, marked as
  heard by that engine alone:
  ```text
  U0234 04:12.1 1.2s | {P:[none]|W:Look out, Luffy!}
  ```
  Adjudication keeps it as dialogue or flags it `DROP` as noise, like any other line. The rule in
  the [pipeline](/documentation/architecture/pipeline.md#5-diff-sheet) asks for two other engines;
  with Whisper as the only other engine, one engine and the language model's judgement decide.
- **Measured first:** over Dressrosa 11–48, the chunks where Parakeet has no word and Whisper has
  some, and what Whisper heard there.
- **Measured after:** those utterances on the sheets of Dressrosa 11 and 28, how many become cues,
  and that no other line changes.

## 2. A glossary prompt for Whisper

Whisper's decoder takes an initial prompt that leans it toward the words in it. The app gives it
none: CrispASR's session API (v0.8.37, pinned) has no initial prompt for Whisper; its hotwords
reach Parakeet only when CrispASR runs Parakeet, and LLM backends. The lower-level `whisper_full`
parameters in `crispasr-sys` do take an initial prompt, so the item needs either Whisper driven
through that API or a CrispASR release that exposes the prompt in its session.

- **What changes:** the job's glossary (the built-in One Piece names, or the job's own file) is
  passed to Whisper as its initial prompt.
- **What it does not reach:** Parakeet, which hears most of the "Flamingo" lines, runs through
  parakeet-rs 0.3.8, which has no word biasing.
- **Measured:** name disagreements on the sheets of Dressrosa 11 and 28 against the baseline, and
  that Whisper does not start hallucinating glossary words into silence.

## 3. Learned glossary terms in the library

The sign library, `library.redb` in the app's data folder, holds a `signs` table and a `meta`
table today ([sign library](/crates/pipeline/src/library/README.md)). Names fixed once should not
need fixing again in every later episode.

- **What changes:** a `terms` table beside `signs`. A spelling the owner keeps in Check Lines, or
  a Fix It change the owner keeps that respells a name, is recorded there. Jobs queued afterwards
  add the learned terms to their glossary, which reaches adjudication, the novelty check, the
  on-screen translation, and Whisper once item 2 exists.
- **Rules:** the glossary is part of the language-model steps' settings, so a finished job does not
  rerun when the library learns a term; a learned term changes spelling and lets the novelty check
  accept the word as heard, and never adds a word no engine heard.
- **Measured:** the terms learned over a run of episodes, and the name corrections later episodes
  still need.

## 4. Context across first-pass batches

The second pass already shows each re-asked line's settled neighbours as `CONTEXT` lines
([`redecode.rs`](/crates/stages/src/adjudication/redecode.rs)), and the first pass sends 60
utterances at a time. What is missing is context across batch edges: each batch of 60 is sent on
its own, several at once, so the first lines of a batch see no line before them, and the `SPK`
flag (a different speaker than the line before) has nothing to compare with.

- **What changes:** each batch after the first also carries the last few sheet lines of the batch
  before it as `CONTEXT` lines, which the answer must not return.
- **Measured:** the lines that change at batch edges on Dressrosa 11 and 28, and the token cost.

## 5. Cleaner audio for the re-decode

Lines flagged `UNSURE` are heard again on the vocal stem, padded by 0.5 s on each side.

- **What changes:** the padded slice is normalised in loudness, and its consonant band (3 to 8 kHz,
  the top of what a 16 kHz stem carries) is raised before both engines hear it again.
- **Measured:** the lines still unsure after the re-decode on the episodes that have them, against
  the baseline.

## 6. A third engine for disputed lines

With two engines, every disagreement is one against one, and adjudication chooses.

- **What changes:** a third open engine, such as Qwen3-ASR-1.7B or Canary
  ([speech recognition landscape](/documentation/research/speech_recognition_landscape.md)),
  hears only the disputed utterances, and its words join the sheet as one more hypothesis. It
  needs an exported model on a runtime law 4 allows, within 5.5 GB of VRAM, in its own worker.
- **Measured:** disagreements settled, unsure lines and Claude calls on Dressrosa 11 and 28.

## 7. Separation quality

Separation reads the audio at 44.1 kHz stereo and writes 16 kHz mono stems, the rate every model
after it takes. The first-pass speech engines hear the original mix; the vocal stem feeds voice
detection, alignment, the vocal half of the sound events and the re-decode. Mel-Band RoFormer
(11 s windows, an 8 s step) is the default; MDX-Net Voc_FT is built too.

- **What changes:** an ensemble of the two built separators, their vocal estimates combined per
  frame; more overlap between windows (a shorter step); HTDemucs v4 only if an exported ONNX model
  exists. The extra RAM holds the second model's windows; the cost is GPU time.
- **Measured:** separation time and VRAM; words timed by the aligner (the report's words by timing
  source), voice detection and sound cues on Dressrosa 11 and 28 against the baseline.

## 8. Speaker turns

The language model marks a line `SPK` when its speaker differs from the line before; nothing in
the pipeline hears voices. parakeet-rs, already linked, includes NVIDIA Sortformer speaker
diarization (up to eight speakers, as speaker turns, not names).

- **What changes:** a diarization pass over the vocal stem gives speaker turns that adjudication
  sees beside the sheet, and that the speaker labels in the roadmap's Later list could build on.
- **Needs:** an exported Sortformer ONNX model to download (the app converts a model only when a
  measurement shows it pays).
- **Measured:** `SPK` flags against the turns on Dressrosa 11 and 28, and the owner's judgement.

## Boundaries

- Depends on: [diff sheet](/crates/stages/src/diff_sheet/README.md),
  [adjudication](/crates/stages/src/adjudication/README.md),
  [CrispASR](/crates/inference/src/ggml/crispasr/README.md), and the
  [sign library](/crates/pipeline/src/library/README.md).
- Used by: roadmap milestone M7.
- Rules: never invent dialogue; every subtitle word comes from what a speech engine heard; models
  are downloaded already exported; each GPU worker within 5.5 GB of VRAM.

## Related documentation

- [Roadmap](/documentation/roadmap.md#m7--dialogue-accuracy-and-audio-ensembling) — milestone M7.
- [Pipeline](/documentation/architecture/pipeline.md) — separation, the diff sheet and
  adjudication.
- [Speech recognition landscape](/documentation/research/speech_recognition_landscape.md) — engine
  benchmarks.
