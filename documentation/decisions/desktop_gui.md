**Status:** live

# Decisions: desktop GUI

The decisions the desktop window led to: how it plays clips and hands videos to other programs,
where models live and how they arrive, when a job passes the quality check, and how the owner's
corrections are timed. The [decision log](/documentation/decisions/) says how entries are
written.

### 2026-09-26 — Clips play through FFmpeg, not libmpv

**Context:** Reviewing a flagged line means hearing it, and seeing who speaks. The GUI feature
named libmpv for the picture, which the app would link; the app may run only FFmpeg, ffprobe and
`claude`, and no audio crate is in the tree (cpal would link ALSA).

**Decision:** FFmpeg plays a clip itself: one FFmpeg sends the clip's sound to PipeWire through its
`pulse` output device, and a second pipes small raw RGBA frames (about 480 lines high) that the
window draws, paced to the start of the sound. No new program runs and no native library links.

**Consequences:** The picture is small and a clip cannot be sought inside; it is played again from
its start. The machine check reports an FFmpeg without the `pulse` device, where clips play
without sound.

**Supersedes:** none.

### 2026-09-26 — The desktop portal chooses files and opens videos

**Context:** The queue needs a file and folder chooser, and the report a button that opens the
video in VLC. Starting VLC would be a fourth external program; `rfd` loads `libdbus` at run time.

**Decision:** The app asks the XDG desktop portal over D-Bus, through `ashpd` (zbus, pure Rust,
async-io): its FileChooser for videos, folders and glossary files, and its OpenURI to open a video
in the desktop's default player, VLC on the owner's PC, or a folder in the file manager. The app
starts no program for either.

**Consequences:** The dialogs are the desktop's own. The portal cannot start a video at a given
time, so the report names the flagged times and the review plays its own clips.

**Supersedes:** none.

### 2026-09-26 — The 120-minute test joins three episodes

**Context:** The pipeline's acceptance asks for a 120-minute test file; no video in the media
folder is longer than 43.6 minutes.

**Decision:** Dressrosa 39, 48 and 40 are joined by stream copy with FFmpeg's concat demuxer into
one 128.9-minute file under `~/.local/share/tbd-subtitles/test-media/`. The episodes are only read
and the media folder is not touched.

**Consequences:** The test runs the same encoding as the batch. Its result is the
[120-minute test](/documentation/research/long_video_120min.md).

**Supersedes:** none.

### 2026-09-26 — The owner's corrections are timed by a review step

**Context:** A corrected line must be timed again and the subtitle file rewritten. Running
alignment again over the whole job needs the GPU, which a batch job holds, and may move the lines
around the corrected one.

**Decision:** An eighteenth step, `review`, runs after alignment. It reads the owner's corrections
from `review.json` and aligns each corrected utterance alone with Parakeet-CTC on the CPU, in a
worker of the main binary; every other line keeps its times. Cues, the quality check and the output
then run again. Corrections survive any later run of alignment.

**Consequences:** A correction takes seconds and runs beside a batch job. Text the owner types is
the one exception to "every subtitle word comes from what a speech engine heard": the owner heard
it.

**Supersedes:** none.

### 2026-09-26 — A short interjection shares a neighbour's cue

**Context:** The 120-minute test left three one-word cues under 20 frames ("ha!", "Ah!", "Huh?"),
each between two cues too full to share one with; the layout rule forbids them, so such an
episode fails the quality check.

**Decision:** When a cue stays too short after the sharing rule, its text is merged into the cue
before or after it, the joined text broken again into at most two lines of 42 characters within
the reading speed; when no merge fits, the short cue grows into the gaps around it down to the
2-frame gap.

**Consequences:** The cue stage's revision rises, so every job redoes its cues, quality check and
output once.

**Supersedes:** none.
