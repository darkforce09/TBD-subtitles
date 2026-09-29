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

### Correction run

A short job the window queues when the owner saves, keeps or takes back a line in Check Lines,
or when [Fix It](#fix-it) changed lines: the `review` step times the corrected lines again on the
CPU, then the cues, the quality check and the subtitle file are rebuilt. It is not a full run
of the video, and it shows in its video's sidebar row as "Updating subtitles".

In code: `JobKind::Review` in `apps/tbd_subtitles/src/job_queue/models/queue.rs`, which calls it
a review run; `StepName::Review` in `crates/job_model/src/stage/step_name.rs`.

See: [desktop GUI](/documentation/features/gui.md#overview-to-check-lines),
[pipeline](/documentation/architecture/pipeline.md#7-forced-alignment)

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

### Fix It

The button on a finished job's Overview that has a stronger `claude` model (Opus by default) fix
the lines the quality check flagged: it reads the whole video for its context, fixes one kind of
problem at a time, and keeps a change only when a judge accepts it and every word was heard by a
speech engine. Each change becomes a correction the owner keeps or undoes in Check Lines.

In code: `crates/stages/src/fix_it/` and `crates/pipeline/src/fix_it/`; `Chosen::FixIt` in
`crates/job_model/src/outputs/review.rs`.

See: [Fix It](/documentation/features/fix_it.md)

### Folder kind

The type of a folder under the README standard — area root, crate root, domain, leaf,
command-line, data or documentation folder — which decides the sections its README adds.

See: [README standard](/documentation/standards/readme_standard.md#kinds)

### Forced alignment

Finding when each word of a known text is spoken in the audio. Gives the final word timings.

### Gate

A repository check that a program runs over the tracked files: `cargo gates <gate>`. Each gate
ends in exit 0 (every check held), 1 (a violation) or 2 (a check that could not run).

In code: `tools/repo_gates/`; each gate is a variant of `Gate` in `tools/repo_gates/src/cli.rs`.

See: [Verdict](#verdict), [coding standards](/documentation/standards/coding_standards.md)

### GGUF, ONNX, safetensors

Model file formats: GGUF for ggml-based runtimes, ONNX for ONNX Runtime, safetensors for candle
and burn. Downloaded ready-made; never converted by us.

### Hand-off

What a later launch of the app sends the window already open: its videos, whether to start the
queue and whether to raise the window, as one JSON line over a local socket. The later launch
then exits, so only one window runs.

In code: `HandOff` in `apps/tbd_subtitles/src/core/single_instance.rs`.

See: [automation](/documentation/features/automation.md#one-window)

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

One part of the pipeline with typed inputs and outputs in the job's work directory, run as one or
more steps.

See: [pipeline](/documentation/architecture/pipeline.md#stage-flow)

### Step

The unit the job runner runs, resumes and times: one output, one fingerprint, one row of time and
peak memory in the job report. Speech recognition is one step per engine; adjudication is its
first pass, the re-decode per engine, the second pass and the choice of sound cues.

In code: `StepName` in `crates/job_model/src/stage/step_name.rs`.

See: [pipeline](/documentation/architecture/pipeline.md#steps-and-processes)

### Stem

One part of a separated mix: the vocal stem (voices) and the background stem (music and effects).

### TDT

Token-and-duration transducer, the decoder design of NVIDIA's Parakeet models; it predicts each
token and how long it lasts, which gives word timestamps directly.

### Timing source

What timed a displayed word: the CTC aligner over a block, the aligner over the utterance alone,
the backbone engine's own time, or interpolation between timed neighbours. The job report counts
the words per source.

In code: `TimingSource` in `crates/job_model/src/outputs/aligned.rs`.

### UNSURE

The adjudication flag on a line whose words stay doubtful after re-decoding; listed with its
timestamp in the job report for review.

### VAD

Voice activity detection: marks where speech is present. Used to cut audio into chunks and to
find speech left without a cue.

### Verdict

The outcome of one check: held, failed, or did not run. A check that could not read its input
or run its program did not run, and never counts as a pass.

In code: `Verdict` and `NotRun` in `tools/verification_core/src/verdict.rs`.

### Watch folder

A folder the owner lists in Settings, Automation. While the app is open, each video in it or its
subfolders that has finished downloading, has no subtitles and was never queued is queued, and
the queue starts.

In code: `apps/tbd_subtitles/src/job_queue/services/watch_scan.rs`; `watch_folders` in
`settings.toml`.

See: [automation](/documentation/features/automation.md#watch-folders)

### WER

Word error rate: the share of words substituted, deleted or inserted against a reference
transcript. Lower is better.

### Worker process

A `worker <step>` subcommand of an app binary that runs one step in its own process and exits
when done, freeing VRAM. The main binary `tbd-subtitles` hosts the ONNX Runtime, FFmpeg and
`claude` workers; `tbd-subtitles-ggml` hosts Whisper, because ggml and ONNX Runtime cannot share
a process.

### Work directory

The folder of one job, where every stage writes its output and from which a resumed job reads
what earlier stages wrote. Source videos are never written to.

See: [system overview](/documentation/architecture/system_overview.md#job-work-directory)
