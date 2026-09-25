**Status:** live

# Glossary

The terms these documents use, one heading each. Documents link a term's first use here.

### Adjudication

The language-model step that turns several speech engines' hypotheses into one transcript: it
picks between heard variants, fixes spelling from the name glossary, punctuates, and flags doubt.
It may not add words no engine heard.

### ASR

Automatic speech recognition: audio in, text out. Here always with word timestamps.

### ASS

Advanced SubStation Alpha, a subtitle format with styles and positioning (`{\an8}` top centre,
`\pos(x,y)` exact). Needed for sign subtitles placed near on-screen text.

### Backbone engine

The speech engine whose word sequence the other engines' hypotheses are aligned against when the
diff sheet is built. Parakeet by default.

### CPS

Characters per second: a cue's character count divided by its duration. The reading-speed limit is
20 for adult English.

### CTC

Connectionist temporal classification: a model output with one label probability per audio frame.
Forced alignment runs a Viterbi search over CTC outputs to place known words in time.

### Cue

One subtitle event: start time, end time, one or two lines of text.

### Diff sheet

The compact per-episode listing of utterances in which words the engines agree on are plain and
disagreements appear as `{A|B|C}` slots. The input to adjudication.

### Dub

An audio track re-recorded in another language. The Muhn Pace videos carry the English dub, whose
script differs from the Japanese version's subtitles.

### Forced alignment

Finding when each word of a known text is spoken in the audio. Gives the final word timings.

### GGUF, ONNX, safetensors

Model file formats: GGUF for ggml-based runtimes, ONNX for ONNX Runtime, safetensors for candle
and burn. Downloaded ready-made; never converted by us.

### Hypothesis

One speech engine's transcript of a stretch of audio.

### Muhn Pace

A fan re-edit of One Pace that uses the English dub audio.

### One Pace

A fan project that re-edits the One Piece anime to follow the manga's pacing; it releases
Japanese-audio versions with English subtitles.

### OP / ED

Opening and ending theme songs.

### Orphan

Speech that the backbone engine missed but at least two other engines heard; recovered as a new
utterance.

### SDH

Subtitles for the deaf and hard of hearing: dialogue plus sound cues in brackets, and speaker
labels where needed.

### Shot change

A cut between camera shots. Professional timing snaps cue starts and ends to nearby shot changes.

### Sign

On-screen text (a signboard, letter, title card) and the subtitle that translates it.

### Sound cue

A bracketed description of a meaningful non-speech sound in SDH, lowercase: `[explosion]`,
`[laughs]`.

### Stage

One step of the pipeline with typed inputs and outputs in the job's work directory. Stages are
resumable.

### Stem

One part of a separated mix: the vocal stem (voices) and the background stem (music and effects).

### TDT

Token-and-duration transducer, the decoder design of NVIDIA's Parakeet models; it predicts each
token and how long it lasts, which gives word timestamps directly.

### UNSURE

The adjudication flag on a line whose words stay doubtful after re-decoding; listed with its
timestamp in the job report for review.

### VAD

Voice activity detection: marks where speech is present. Used to cut audio into chunks and to
find speech left without a cue.

### WER

Word error rate: the share of words substituted, deleted or inserted against a reference
transcript. Lower is better.

### Worker process

A subcommand of the app binary that runs one GPU stage in its own process and exits when done,
freeing VRAM.
