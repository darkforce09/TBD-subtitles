**Status:** live

# High-accuracy dialogue: audio ensembling and orphan recovery

How speech recognition accuracy advances from 95% to 99%+ by eliminating the diff-sheet
blind spot, biasing Whisper against phonetic mishears, learning arc glossaries across
episodes, and validating acoustic disagreements.

## The remaining 5% of dialogue errors

Validation across 50 episodes shows two primary error modes in dialogue generation:

1. **Occasional missing dialogue:** short shouts, single-word interjections (*"Run!"*,
   *"Look out!"*), or faint whispered lines present in the audio are missing from the output.
2. **Proper-noun phonetic drift:** approximately five names, attacks, or arc terms per episode
   are misspelled or substituted with standard English dictionary words (*"Bartholomew"*
   for *"Bartolomeo"*, *"Flamingo"* for *"Doflamingo"*).

Both modes stem from architectural assumptions in the diff sheet and ASR prompt layers.

## 1. Symmetric orphan recovery in `diff_sheet`

In [`crates/stages/src/diff_sheet/sheet.rs`](/crates/stages/src/diff_sheet/README.md), the
backbone engine (Parakeet) drives utterance segmentation. If Parakeet hears zero words in a
speech chunk:

- `cut_points(&chunk.words)` returns an empty vector.
- The chunk iteration terminates immediately without calling the emission closure.
- Even if Whisper heard complete dialogue with high confidence, that speech is silently discarded.

### The solution

`diff_sheet` is made symmetric across engines:

- When the backbone has zero words in a chunk or silence window where a secondary engine heard
  speech, an **orphan candidate** is generated:
  ```text
  U0234 04:12.1 1.2s | {P:[none]|W:Look out, Luffy!}
  ```
- The candidate is handed to Claude adjudication with the surrounding dialogue context. Claude
  determines whether the candidate represents real spoken dialogue or noise.
- Spoken interjections and quiet lines are preserved even when the primary engine misses them.

## 2. Dynamic arc prompt biasing in Whisper

Whisper features an `initial_prompt` parameter that primes its decoder with context, style,
and vocabulary. Currently, CrispASR provides Whisper with no prompt, running it completely cold.
Whisper defaults to common English vocabulary over phonetically similar anime terminology.

### The solution

Before transcribing an episode, the runner constructs an arc vocabulary prompt:

```text
One Piece, Dressrosa, Straw Hat Luffy, Doflamingo, Bartolomeo, Cavendish, Rebecca,
Kyros, Bellamy, Corrida Colosseum, Haki, Law, Trebol, Diamante, Pica, Senor Pink...
```

- Injected into Whisper's decoder via CrispASR.
- Biases beam-search token probabilities toward canonical spellings.
- Eliminates the majority of proper-noun mishears before they enter the diff sheet.

## 3. Self-learning glossary in `library.redb`

Fixed terms should not need manual correction more than once across a 40-episode arc.

### The solution

`library.redb` maintains a dynamic series dictionary table:

1. When the owner accepts a corrected term in Check Lines, or when Fix It resolves an `UNSURE`
   novel word, the confirmed spelling commits to `library.redb`.
2. Subsequent episodes automatically draw their Whisper prompt bias and Claude system glossary
   from this database table.
3. Every processed episode enriches the accuracy of all remaining queued episodes.

## 4. Conversational context windows in adjudication

Currently, Claude adjudicates utterances in fixed windows without historical continuity. A
character name spoken in isolation can be ambiguous.

### The solution

Claude receives the **preceding three settled utterances** as dialogue context. If an
utterance is preceded by Zoro shouting at an opponent, Claude leverages that narrative context
to resolve ambiguous phonetic tokens against the scene's active characters.

## 5. Selective vocal stem re-decoding with dynamic gain

Utterances marked `UNSURE` undergo re-decoding on the vocal stem. In loud battle sequences,
vocal separation occasionally attenuates vocal transients.

### The solution

Before re-decoding:
- Apply loudness normalization to the target 0.5-second padded vocal window.
- Apply high-pass pre-emphasis to elevate consonant frequencies (3 kHz to 8 kHz).
- Whisper and Parakeet re-evaluate the enhanced vocal slice with clearer consonant definition.

## 6. Third ASR engine acoustic voting

With two engines (Parakeet and Whisper), any disagreement represents a 1-versus-1 tie, forcing
Claude to guess which engine misheard.

### The solution

A lightweight third open ASR engine (such as `Qwen3-ASR-1.7B` or `Canary-1B`) runs on
disputed utterances:

- Provides 2-out-of-3 acoustic consensus voting.
- Resolves acoustic ties before language-model adjudication.
- Drastically reduces the frequency of `UNSURE` flags and unnecessary Claude API calls.

## Boundaries

- Depends on: [`diff_sheet`](/crates/stages/src/diff_sheet/README.md),
  [`adjudication`](/crates/stages/src/adjudication/README.md), and
  [`library.redb`](/documentation/architecture/binary_storage_plan.md).
- Used by: roadmap milestone M7, the diff-sheet stage, and worker runners.
- Rules: never invent dialogue; all subtitle words originate from speech heard by an engine.

## Related documentation

- [Roadmap](/documentation/roadmap.md) — milestone M7.
- [Pipeline](/documentation/architecture/pipeline.md) — diff sheet and adjudication.
- [Speech recognition landscape](/documentation/research/speech_recognition_landscape.md) — engine benchmarks.
