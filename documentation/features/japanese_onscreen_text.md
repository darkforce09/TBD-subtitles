**Status:** live

# Japanese on-screen text

The visual translation pipeline and Check Text interface are implemented; M4 acceptance is still
underway. A video job produces one ASS file containing dialogue, sound cues and English for
visible Japanese writing. With Replace text in the video on (the default for new jobs), it also
writes a [localized video](/documentation/glossary.md#localized-video): a copy with the Japanese
erased and the English drawn in its place where that can be done cleanly, and a subtitle file of
its own with the dialogue and sound cues. That part is M5, implemented and under validation, not accepted. The
source video is only read. Pilot coverage, full-episode quality, resource limits, packaged
playback and owner acceptance are not yet established by this document.

The owner accepts missed faint text and false detections as current limitations. The AppImage
is built and passes its host startup smoke check. Detection screens sampled frames at full
resolution and bisects the exact boundaries, and Claude reads each text event once from its
keyframe; the host measurement of full-resolution screening and of the segment encode of the
localized video
([runbook](/documentation/runbooks/measuring_full_resolution_screening.md)), full GUI correction
acceptance and VLC playback remain open.

## Where it lives

- The ten visual steps: [on-screen text stages](/crates/stages/src/onscreen_text/), orchestrated
  by [visual pipeline tasks](/crates/pipeline/src/tasks/onscreen.rs) and, for the three
  replacement steps, [replacement tasks](/crates/pipeline/src/tasks/replace.rs) over the
  [replacement stages](/crates/stages/src/onscreen_text/replace/), and for the read-back check,
  its [task](/crates/pipeline/src/tasks/verify.rs).
- The localized video: the [localize stage](/crates/stages/src/localize/) and its
  [task](/crates/pipeline/src/tasks/localized.rs), with FFmpeg's [encode](/crates/media_io/src/encode/)
  and LaMa in the [inference crate](/crates/inference/src/onnx/lama/). The architecture is in the
  [video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md).
- The sign library shared by episodes: [library](/crates/pipeline/src/library/).
- Detection and reading: [OCR backends](/crates/inference/src/ocr/); translation:
  [language-model backends](/crates/inference/src/llm/) and the isolated
  [local-model worker](/apps/tbd_subtitles_llm/).
- Desktop review: [Check Text](/apps/tbd_subtitles/src/text_review/), the
  [application actions](/apps/tbd_subtitles/src/application/actions/text.rs), and
  [On-screen Text settings](/apps/tbd_subtitles/src/settings/ui/onscreen_text.rs).
- Entry: a normal video job in the [desktop GUI](/documentation/features/gui.md) or
  `tbd-subtitles process <video>`. There is no separate visual-translation queue.
- Related: the [pipeline](/documentation/architecture/pipeline.md),
  [subtitle formats](/crates/subtitle_formats/) and
  [validation harness](/tools/visual_validation/).

## Behaviour

### Scope and settings

New jobs enable on-screen translation. The scope includes all readable visible Japanese: signs,
title and name cards, letters, posters, credits, decorative writing and visible lyrics. Detection
coverage is an acceptance target, not a guarantee that the models find every occurrence. Japanese
words that are only audible remain outside this visual feature.

Settings → On-screen Text provides:

- **Translate on-screen text**, enabled for new jobs. When enabled, the effective output format is
  ASS even if General selects SRT or WebVTT. The ASS also contains the job's ordinary subtitles.
  Its help says the original video is never changed.
- **Replace text in the video**, on for new jobs and for a settings file saved before the switch
  existed; off to clicks while translation is off ("Turn on Translate on-screen text to use it.").
  Its help: the Japanese is erased and the English drawn into a copy of the video,
  `<name>.localized.mkv`, saved beside the original, whose subtitles go in `<name>.localized.ass`.
  Turning it on adds the LaMa inpainting model and the Latin fonts to the required models.
- **Claude fallback**, enabled by default. The installed, signed-in Claude CLI reads and
  translates every detected text event once from its keyframe, under the existing shared call
  limit with structured replies; local OCR and Qwen answer whatever it leaves. Off, or with the
  CLI unavailable, the local models supply the result and compatible reference wording can
  improve it. There is no paid API backend or automatic billing fallback.
- **Detector engine: TensorRT / CUDA**, CUDA by default until the host bench confirms TensorRT.
  It runs both the screening and the confirming detector; TensorRT builds an FP16 engine the first
  time for each GPU, driver, TensorRT version, model and input shape, and reuses it from the app
  data folder after. The detection step's fingerprint covers the choice, since the results differ
  between engines.
- **Decode video on the GPU (NVDEC)**, off by default, so the CPU decodes. It changes only how the
  screen's frames are decoded, which is bit-exact either way, so switching it reruns nothing.
- **Video encoder**, x264 (the default) or NVENC, for the segments the localized video
  re-encodes; the localized video's fingerprint covers it.
- **Reference subtitles**, an editable folder path. The reference loader reads ASS files directly
  in that folder; it does not recursively scan the media library. No path means no references.
- **Model availability**, with Open Models and Download Missing. Missing models prevent a job
  from starting and are listed in the existing model banner. Claude readiness links to This
  Computer; a missing or unsigned-in CLI leaves an actionable warning on uncertain text.

Changes save as they are made. They apply to jobs that have not started. Saved job records without
visual settings keep visual processing disabled, and records without the localized-video setting
keep the localized video off; opening or resuming them does not silently add a new scan or a
re-encode. Run Again with Current Settings explicitly adopts the current settings for that video.

### One combined job

After dialogue cue construction, the job runs ten resumable visual steps before final quality
checking and output, and writes the localized video last:

```text
Dialogue cues -> Detect -> Read -> Track -> Translate -> Review -> Mask -> Inpaint -> Compose
  -> Verify -> Typeset -> QC -> combined ASS (+ localized ASS) -> localized video
```

Mask, Inpaint, Compose, Verify and the localized video are described under
[replacement in the video](#replacement-in-the-video); with the localized video off they record
that they are off and do nothing.

| Step | Work and retained result |
|---|---|
| Detect | Streams every frame at full resolution as 8-bit YUV through FFmpeg with packet presentation timestamps. Screens every `round(fps / 2)`-th frame plus the first and last frame of each shot with the mobile PP-OCRv5 detector on two sessions, and bisects the frames between two samples to the exact frame where writing appears or vanishes. Keeps one observed frame per sample or boundary, a keyframe for each occurrence (the screened sample nearest its middle), confirmed by the server PP-OCRv5 detector at full resolution, and a perspective-corrected crop from that keyframe; cuts or changed writing start new occurrences. Screening noise is dropped: writing shorter than 0.15 s, wider than half the frame, or absent from its keyframe. |
| Read | Reads Japanese with PP-OCRv5 through oar-ocr, using manga-ocr for difficult crops. Consolidates compatible adjacent readings and folds furigana into the kanji line they annotate; uncertain readings remain flagged. |
| Track | Checks that every sampled box keeps its centre (within a fifth of the keyframe box height) and half its overlap with the keyframe quad, without decoding video; a detector box that only grows or shrinks around unmoved writing passes. A moving or unverified surface gains a review warning and nearby placement. |
| Translate | Asks Claude first, one call per keyframe frame with the whole-frame still and the crops of its regions, for each region's Japanese, English, confidence and box plus any other writing on the frame. Qwen3.5-4B, with short dialogue context and the glossary, loads only for occurrences Claude leaves unanswered; compatible corrected occurrences consolidate, and occurrences that show one sign join into one, before review. |
| Review | Checks a saved correction's source identity before applying the owner's English, timing and presentation independently of dialogue corrections, then joins the occurrences that show one sign again. |
| Typeset | Produces ASS text or vector glyph events, with stable typography and frame-specific geometry. Unsafe replacement uses nearby English with a review warning. These events go into the combined ASS alone; the localized subtitle file has none. |

Claude image requests run as many at once as the job's `claude` process count, before any local
model loads; requests for the same keyframe share a cache lock, and the existing Claude process
cap, cancellation and call logging apply. Every hinted region must come back exactly once, or its
occurrences fall to the local phase; a region whose box lands away from its hint is kept with a
review warning; other writing Claude reports on the frame becomes a new occurrence with the
surrounding event's timing, nearby placement and a review warning. Qwen runs through mistral.rs in
`tbd-subtitles-llm`, separate from the ONNX Runtime processes, and only for occurrences without
a valid Claude answer. Readings without Japanese script, or with OCR confidence below 0.5, bypass
Qwen and remain unresolved. The shared GPU lock prevents simultaneous GPU stages. The model store
downloads pinned exported models; the only local compilation is the TensorRT engine the runtime
builds from the detector exports when that engine is chosen.

Detection samples the full-resolution stream at two frames per second and at every shot
boundary, so writing visible for fewer frames than the sample step that falls between two samples
and touches no cut is missed. Each sample is screened twice: at full resolution, so small or
faint writing has the source's pixels (the owner accepts the extra noise that brings), and shrunk
to 640 wide, where writing too large for the mobile detector at full resolution (a single glyph a
third of the frame tall) is found; a 640-wide box is kept only where the full-resolution boxes
cover less than half of it
([decision](/documentation/decisions/onscreen_detection.md#2026-10-01--screening-adds-a-640-wide-pass-for-writing-too-large-at-full-resolution)). A sample whose luma, in
32 × 32 blocks, matches the last screened sample's within a mean difference of 4 reuses its
detections. The rest are converted to RGB, padded with black rows to a multiple of 32 lines
(1,088 for a 1080p source, never stretched), and screened in batches on two detector sessions in
the one worker, each on its own thread; bisection probes go ahead of waiting batches, and results
are applied in sample order, so the document is the same with one session or two. The batch and
each session's GPU memory pool are one measured pair, set by the host sweep
([decision](/documentation/decisions/onscreen_detection.md#2026-10-01--the-detector-screens-full-resolution-frames-padded-to-a-multiple-of-32-on-two-sessions)).
Detection compares each candidate with the occurrence's fixed first signature, read from the
luma plane, rather than allowing small changes to accumulate against successive samples.
Matching requires a unique association in both directions. The signature allows small alignment jitter but checks
individual pixel differences, 8 by 8 cells and the whole crop, including its edges; a changed
glyph in a long line or newly visible scrolling text can therefore split the occurrence, and the
bisection finds the exact frame of that change. Perspective correction makes slanted crops
upright for recognition. Surface-colour safety is measured on the full-resolution keyframe, so
that correction cannot turn an unsafe background into permission to cover it.

Each occurrence's keyframe is the screened sample nearest its middle, so at 24 fps it lies within
six frames of the exact middle. While writing is on screen, the scan keeps the samples that can
still be its middle in memory, within a budget of 16 GiB; after the scan, the server detector
confirms every occurrence on its keyframe at full resolution, spread over both sessions. A
keyframe let go to keep the budget is decoded again by FFmpeg as a still, eight at a time
([decision](/documentation/decisions/onscreen_detection.md#2026-10-01--the-server-detector-confirms-each-occurrence-at-full-resolution-on-the-sample-nearest-its-middle)).
The step's notes in the report time the warm-up, decode wait, conversion, screening, probes,
signatures and confirmation, and count stills from memory against stills from FFmpeg.

Adjacent occurrences consolidate only when Japanese readings, geometry and timing agree, with at
most one observed source-frame gap and no known cut or ambiguous match; half a frame of slack
absorbs timestamps rounded to the microsecond, and the earlier interval covers the missing frame.
Once translated, English must also agree and both confidence scores must be on the same side of
the 0.85 display threshold. This keeps faint fragments separate from a confidently translated
interval. A merged interval retains every measured frame, representative crop, warning and
provenance note, with the lowest confidence of its parts.

Furigana, the small kana printed over kanji to give their reading, fold into the one kanji line
they sit on: the kana line is 0.18 to 0.55 of the line's height, its bottom lies between half a
line height above the line's top and a third of a line height into it, its centre stays over the
line (a tenth of a line height past either end at most), it is at most 1.6 times as wide as the
line, and at least half of its time falls inside the line's; the position must hold for two
thirds of that shared time, so a detector box that briefly covers only part of the line does not
break the pair. The line keeps its reading, English
and geometry; the kana box at the line's keyframe is recorded as its ruby and erased with it, and
the kana is never translated or lettered on its own. A kana line that fits two kanji lines stays
separate.

One sign can be observed several times: by the detector, by Claude on each keyframe it reads, and
in pieces around a short detection miss. After translation, and again after owner corrections in
the review step, occurrences join when they show the same Japanese (equal once spaces are removed
and small kana are read as full-size, or one reading of at least two characters inside the
other), their boxes at the keyframe overlap
by at least 0.3 or one holds the other's centre within a quarter of its height, and their spans
overlap or pause at most 0.25 s with no cut in the pause. A reviewed occurrence survives and keeps
the owner's timing and English; otherwise the detector's occurrence survives over writing Claude
found, then the longer span and the higher confidence. Two reviewed occurrences never join. The survivor covers the
joined span with frames that leave no hole, using its own keyframe box where only Claude saw the
sign, takes the English with the highest confidence, and notes `Same writing as <ids>` in its
reason; furigana grouping then runs again. Re-running the review step repairs an existing job
without Claude calls.

A local translation of a reading containing at least four katakana characters and kanji requires
independent image verification: its local confidence is capped at 0.84. This conservative check
helps catch shortened compound names and qualifiers. Without available Claude verification it
stays flagged and unrendered until reviewed; the check does not establish translation completeness.

Frame buffers, thumbnails and preview streams are bounded: the scan holds a decode queue of about
four seconds of frames from a recycled pool, the gap frames between samples until every
transition that could land on them is resolved, the keyframe candidates within 16 GiB, the
batches in flight on the two sessions, and the packet table; the step waits, before it starts,
for the GPU memory its sessions need within the 6.5 GB worker cap. The scan fails
explicitly beyond one million geometry observations or one hundred thousand occurrences, and when
the decoded frame count differs from the packet count. Each ASS event buffer has a
128 MiB budget: oversized vector lettering for one occurrence uses a flagged nearby label, while
an oversized combined buffer fails explicitly. These bounds do not guarantee a particular process
memory peak or disk usage; retained crops, observations and caches occupy the job's work directory.

Reference translations are conservative. A nearby dialogue anchor must match the dub scene and
the reference sign wording must agree with the independent translation of the visible reading.
Only wording and its source are reused. Placement, geometry and timing come from the current
video. An absent folder falls back to local translation; a mismatched scene supplies no wording.

### Replacement in the ASS file

ASS can draw shapes, text and vector glyphs; it cannot recover artwork hidden behind the Japanese
letters. Replacement is limited to surfaces whose measured colour and track pass the safety
checks. Perspective lettering uses transformed glyph outlines rather than separately generated
images. A static surface keeps the keyframe's exact geometry through its whole interval; a sign
that moves between samples uses nearby placement.

Detailed backgrounds, transparency, uncertain occlusion or geometry that fails those checks keep
the original picture and use nearby English with a warning. The replacement preference in Check
Text does not override those checks. Extending a sign's timing beyond measured tracking uses
nearby placement for the unverified span. If no English can be rendered, the occurrence remains
unresolved and explicitly flagged; English wording alone does not prove that it appears in ASS.

An unreviewed reading or translation with confidence below 0.85, or a non-finite score, produces no
English event. Its candidate wording remains in Check Text with a **No rendered translation**
warning. A saved owner correction can approve that wording for display; surface and placement
safety still apply. Nearby labels reserve the dialogue area and avoid simultaneous labels. If no
readable position is available, the wording remains flagged for review rather than overlapping.

Per-occurrence uncertainty can finish with warnings. Missing required models, failed local-worker
startup, decoding failures and other infrastructure errors fail the step explicitly. A malformed
language-model answer becomes an unresolved result; an unavailable optional Claude fallback is
reported with its reason. Cancel and Try Again operate on the existing video's job and reuse
valid completed stages.

### Replacement in the video

With Replace text in the video on, the job tries every displayable translated occurrence (reviewed,
or at a confidence of 0.85 or more) for replacement in the picture, including writing the ASS file
places nearby because it moves or only Claude found it:

1. **Mask** separates the writing's strokes from their background on the keyframe, measures their
   colour, outline and thickness, follows moving writing frame by frame and divides the span into
   runs of unchanged background, decoding only the region around the writing.
2. **Inpaint** fills the erased strokes of each run's background with the LaMa model, in its own
   GPU worker.
3. **Compose** letters the English in Noto Sans in the writing's place, colour and weight, fitted
   to its area; writing that sits together on one card keeps its size ratio.
4. **Verify** rebuilds a few frames of each lettered occurrence as the localized video will show
   them and reads them back with the local PP-OCRv5 on the GPU: Japanese still readable where the
   writing or its furigana was, or English that does not read back as the translation, leaves the
   occurrence in Japanese. No API call is made.
5. **Typeset** writes the usual ASS events for `<name>.ass`; `<name>.localized.ass` gets no
   on-screen events, and its dialogue moves to the top while English drawn into the video sits
   under it.
6. After the subtitle files, **the localized video** blends the lettering in, with the source's
   audio and chapters and no subtitle stream. For a constant-frame-rate H.264 source it
   re-encodes only the segments with replaced writing, from the keyframe before to the keyframe
   after, as H.264 matching the source (x264 by default, NVENC as the encoder setting), copies the
   rest of the video untouched and joins the pieces; the join is checked (frame count, duration,
   audio sync, clean decoding around each join, the copied bytes), and a failed check, or any
   other constant-frame-rate source, gets the whole video decoded and re-encoded with the GPU's
   HEVC encoder (H.264 in software when it cannot run) at about the source's size; a variable
   frame rate is refused as before
   ([decision](/documentation/decisions/localized_video.md#2026-10-01--the-localized-video-re-encodes-only-the-segments-with-replaced-writing-as-h264-matching-the-source-and-copies-the-rest)).

The result beside the source is `<name>.localized.mkv` and `<name>.localized.ass`; `<name>.ass`
stays the complete subtitle file for the original video. A player loads the `.localized.ass` with
the localized video: it holds the dialogue and sound cues alone, so the on-screen English appears
only drawn into the picture, never as a second set of subtitles.

An occurrence is left in Japanese in the picture, with `Not replaced in the video: <reason>` in
Check Text, when the owner chose Nearby in Check Text, when its strokes
cannot be separated from the background, when it moves in a way that cannot be followed or sweeps
over a quarter of the frame, when its background changes into more than 2,000 runs, when the
English would be smaller than 14 pixels of cap height at 1080p, when the font lacks one of its
characters, or when the finished picture still shows Japanese or its English does not read back
(Check Text shows what the check read under the reason). A variable-frame-rate video fails the localized-video step with that reason; a
`<name>.localized.mkv` the job did not write is never overwritten, and the step asks for it to be
moved away. The report adds a Localized video section: occurrences replaced, fallbacks, the file,
the encoder, the segments and frames re-encoded, the frames copied and, when the whole video was
re-encoded, why. On Dressrosa 11, 15 of 21 candidates were replaced, the Rebecca name card among
them ([measurement](/documentation/research/localized_video_dressrosa_11.md)). Algorithms and bounds:
[video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md).

### Signs shared by episodes

A replacement that read back cleanly, of an occurrence the owner neither kept in Japanese nor
moved to a nearby label, becomes an approved sign in the sign library, `library.redb` in the app's
data folder, shared by every episode: its Japanese, its keyframe crop's hash, its English and
confidence, its lettering style, and its patch and mask. When a later episode shows the same
writing (the same Japanese once full- and half-width forms and spaces are folded, and a crop that
looks nearly the same), translation takes the stored English, shown as `library` in Check Text's
provenance, and asks Claude nothing for a keyframe whose every sign is known; composition starts
from the stored lettering style. The sign is still erased, lettered and read back in its own
frames. An episode never takes its own signs. Saving a correction in Check Text that changes or
removes a sign's English or keeps it in Japanese, or retrying it, removes it from the library.
Settings, On-screen Text, shows the library's size and clears it after asking. With an empty
library a job runs exactly as without one. Design:
[library shared by episodes](/documentation/architecture/binary_storage_plan.md#library-shared-by-episodes).

### Reviewing the result

1. Queue a video normally. Its progress includes the visual steps, their elapsed time and the
   estimate for the combined job. The sidebar keeps one video entry.
2. In Overview, inspect detected, translated, fallback and unresolved text counts ("· 15 replaced
   in the video" once the replacement steps ran), visual processing time and the final ASS path.
   While the localized video is on disk, a card says "Localized video saved next to the
   original" with its path and its subtitle file's, and Open in Player, Show in Folder and Copy
   Path for the video. Check Text opens the visual review directly.
3. In Check Text, select an occurrence by thumbnail and timestamp. The view shows the Japanese
   reading, English, confidence, translation provenance and review status; a filter narrows the
   list to flagged occurrences.
4. Compare Original and English previews, play, scrub or step by one source frame. FFmpeg renders
   the actual exported ASS before reducing the picture for the preview. Unsaved editor changes
   do not appear in that exported preview. For a job with the localized video, a **Subtitles |
   Localized video** control over the right picture chooses between the source with `<name>.ass`
   and the localized video with `<name>.localized.ass` (the default); until the video is written,
   the right picture is the occurrence's replaced keyframe plate, captioned "Localized video not
   written yet". **Show erase mask** beside the original picture's title tints the keyframe's
   [stroke mask](/documentation/glossary.md#stroke-mask) over it. A line under the time says
   "Replaced in the video" in green or `Not replaced in the video: <reason>` in orange, and the
   Replace treatment reads "Replace in the video".
5. Edit the English, start/end times, placement, size or replacement/nearby treatment. **Save &
   regenerate ASS** keeps the result and queues a correction run; **Undo** removes that occurrence's
   saved correction. **Retry selected text** requests another reading and translation for the occurrence.
6. Wait for regeneration and inspect the rendered result again. Changes invalidate affected visual
   stages and final output while valid audio work remains reusable; with the localized video on,
   a correction also redoes the replacement steps and re-encodes the localized video, which takes
   minutes rather than seconds. Each **Retry selected text**
   advances that occurrence's cache generation: it requests fresh work once, and a resumed run
   reuses valid answers from that generation. Another explicit retry advances it again.

A saved correction applies only to the source occurrence it was made against. Changed Japanese,
observed frame times, geometry or crop identity leave the correction unapplied and flagged;
corrections without an identity also need to be saved against the current occurrence.
English wording, edited display times and presentation changes do not alter the retained original
identity. Corrections whose occurrence disappears after reprocessing appear as document warnings,
including when the occurrence list is empty. **Discard unmatched corrections** removes those
orphaned edits and regenerates the output; **Undo** removes an edit on a current occurrence.

Saving takes place off the window thread. A save carries its owning video's path through
navigation and refresh: its completion queues that video's correction run once, even when a
different video is open. Save failures name the original video in an error toast without
overwriting the selected video's session. Loading, preview decoding and model work also stay off
the window thread; closing a preview cancels its child processes.

The existing log window includes OCR diagnostics, local and Claude model calls, visual-step
messages and failures. Completion follows final QC/output after audio and visual processing,
and the localized video after them; there is no separate scan-complete notification. The
job-end notification adds "Localized video saved: `<name>.localized.mkv`." when one was written. Replacing a job's SRT with ASS uses the existing subtitle backup and
retirement mechanism.

## Data

The visual steps keep their documents in the job's database, `job.redb`, beside the audio
stages', and their large files in the same
[work directory](/documentation/glossary.md#work-directory):

| Row or file | Purpose |
|---|---|
| `meta/job_record`, `step_records/<step>` | Job settings, model location and step fingerprints/measurements, including visual processing. |
| `outputs/text_detect` through `outputs/text_review`, and `outputs/text_typeset` | Typed `TextDocument` outputs: readings, tracks, provenance, warnings, source identity, presentation and rendered status, plus review warnings without a current occurrence. |
| `outputs/text_mask`, `outputs/text_inpaint`, `outputs/text_compose`, `outputs/text_verify` | The `ReplacementDocument` each replacement step writes: per occurrence its frame span, status (pending, baked or fallback with its reason), measured lettering style, container and plates; the read-back check's `VerifiedReplacements` adds `checks`, per checked occurrence the frames read and whether all passed. |
| `frames` | One `FrameRecord` per frame of every occurrence `text_mask` gave plates, keyed by occurrence and frame: the quad carried to the frame, its follow score, shift and scale, its erase mask as run-length rows and its plate. Composition, the read-back check and the localized video place each frame's lettering by it. |
| `readings` | One `VerifyReading` per finished frame the read-back check read, keyed by occurrence and frame: the Japanese found, the English read, the similarity and the pass; Check Text shows each occurrence's telling one. |
| `visual/masks/`, `visual/plates/`, `visual/patches/` | Per occurrence: [stroke masks](/documentation/glossary.md#stroke-mask) and source crops, inpainted [plates](/documentation/glossary.md#plate), and RGBA [patches](/documentation/glossary.md#patch) with a `preview.png` for Check Text. |
| `visual/crops/` | Representative full-resolution crops from each keyframe, used by OCR, Claude image requests and review thumbnails. |
| `visual/keyframes/` | One 1280-wide whole-frame still per keyframe frame, sent to Claude with the crops it holds. |
| `visual/readings/` | Cached readings keyed by crop, OCR model pins and retry generation. |
| `visual/translations/` | Cached structured replies keyed by request, model identity/pins and retry generation; a keyframe request also includes the still, its crops and the highest retry generation among its regions. |
| `corrections/text` | Per-occurrence `TextEdit` values with original-source fingerprints and retry requests; each change is one write transaction that reads the row again. |
| `outputs/text_typeset/ass` | Typeset visual events merged into the final ASS. |
| `outputs/localized_video` | The localized video's path, encoder, frames written and occurrences replaced; the segments re-encoded, frames re-encoded and frames copied, and the reason for a whole-video encode; the earlier path while the setting is off. |
| `outputs/output` | The exported subtitle path, the localized subtitle path, and any backup or retired output. |

A file a row names is synced before the row commits, and an open of the database removes the
crops, keyframes, masks, plates and patches no row names; the caches stay.

The [visible-text contracts](/crates/job_model/src/onscreen/) keep visual geometry separate from
dialogue layout. Times use the normalized presentation timeline and positions use source pixels;
the typesetter maps them to the ASS script canvas. Model identities, relevant settings, reference
contents and correction inputs participate in resume decisions. Editing an occurrence does not
change the source video or the line corrections. Beside the source, a job with the
localized video writes `<name>.localized.mkv` and `<name>.localized.ass` next to `<name>.ass`;
watch folders and folder adds never queue a `*.localized.mkv`.

## Design

The feature uses the approved desktop components: one sidebar row, the existing progress and
Overview cards, Check Text beside Check Lines, and an On-screen Text tab in Settings. It has no
separate translation launcher or queue. Text review uses the same palette, typography, controls
and save-and-regenerate pattern as line review.

The output favours readable, reviewable English when clean replacement cannot be justified. The
subtitle file alone cannot restore the background hidden by the Japanese in the supplied board,
title-card and name-card examples, so the localized video edits a copy of the picture: the owner
asked for replacement as Google Translate does it on photographs. Only pixels under a stroke
mask, its one-pixel feather and the new lettering are painted, and only in the copy, whose
segments with replaced writing are re-encoded and whose other frames keep the source's bitstream
(or, when that cannot be done and checked, the whole copy is re-encoded at about the source's bit
rate); the source is only read. Anything the
replacement cannot do cleanly stays Japanese in the picture, with its reason shown in Check Text, rather than
a smeared guess. The localized video carries no subtitle stream, so a player shows the sidecar
`.localized.ass` and never two copies of the same English.

## Open work

- [M4 — Japanese on-screen text](/documentation/roadmap.md#m4--japanese-on-screen-text): complete
  annotated pilot validation for the supplied categories and Dressrosa 11, 16 and 39; every
  readable occurrence should be translated or explicitly flagged, with missed faint text and
  false detections accepted by the owner. Validate credits, visible lyrics,
  vertical writing, brief appearances, fades, perspective, occlusion, repeated signs, cuts and
  mismatched references. The targets are timing within one source frame and accepted tracking
  error within two pixels at 1080p; failed tracks require a flagged fallback.
- The same milestone requires the complete GUI correction flow, VLC playback, a 20–30 minute episode
  benchmark reporting added visual time and compliance with 24 GB RAM / 6.5 GB worker VRAM,
  packaging and host smoke checks, and explicit owner acceptance of the UI and playback.
  Acceptance remains open; unit tests and the harness do not establish these media-level results.
- [M5 — In-place on-screen text](/documentation/roadmap.md#m5--in-place-on-screen-text): on
  Dressrosa 11, 6 of 21 candidates fell back as "could not be separated from its background":
  a title logo whose box spans its artwork and boxes Claude placed away from their writing. The
  localized video still needs a playback check in VLC and mpv, an AppImage rebuild with its host
  smoke test, and the owner's acceptance.
- [M6](/documentation/roadmap.md#m6--24-gb-workstation-throughput) and
  [M8](/documentation/roadmap.md#m8--visual-tracking-and-video-acceleration): full-resolution
  screening, the detector engine, NVDEC and the segment encode are built and await the host
  measurement in the
  [runbook](/documentation/runbooks/measuring_full_resolution_screening.md): the pool and batch
  sweep, the determinism check, the encode presets, fresh runs of Dressrosa 11 and 28 against the
  [M6 baseline](/documentation/research/m6_baseline.md), and playback across every join.

## Decisions

- Local detection finds and times the writing; Claude reads it. The chosen OCR exports are
  documented by [oar-ocr](https://github.com/GreatV/oar-ocr/blob/main/docs/models.md), and
  difficult Japanese crops use [manga-ocr](https://github.com/kha-white/manga-ocr). Exports are
  pinned in the repository's model manifest and downloaded already exported; with the TensorRT
  engine, the runtime compiles its FP16 engines from them
  ([decision entry](/documentation/decisions/inference_engines.md#2026-10-01--tensorrt-runs-the-pp-ocrv5-detectors)).
- Sampled screening with bisected boundaries and one Claude call per keyframe: the
  [decision entry](/documentation/decisions/stack_and_pipeline.md#2026-09-29--on-screen-text-is-found-by-sampled-screening-with-bisected-boundaries-and-read-once-per-event-by-claude-vision);
  full-resolution frames on two sessions, keyframes confirmed at full resolution and the cuDNN
  search mode: the
  [on-screen detection decisions](/documentation/decisions/onscreen_detection.md).
- [ASS positioning, transforms and vector drawing](https://aegisub.org/docs/latest/ass_tags/)
  keep the source video intact and allow one file to carry dialogue, sound cues and signs.
- Writing is replaced in a localized video beside the source: LaMa through ONNX Runtime, Noto Sans
  through tiny-skia, no embedded subtitles, `<name>.localized.ass` beside it, on by default for
  new jobs
  ([decision entry](/documentation/decisions/stack_and_pipeline.md#2026-09-30--writing-is-replaced-in-a-localized-video-re-encoded-beside-the-source));
  only the segments with replaced writing are re-encoded, as H.264 matching the source, with the
  full `hevc_nvenc` re-encode as the fallback
  ([decision entry](/documentation/decisions/localized_video.md#2026-10-01--the-localized-video-re-encodes-only-the-segments-with-replaced-writing-as-h264-matching-the-source-and-copies-the-rest)).
- References contribute verified wording, never unchecked timing or placement from a different
  edit. Unreadable content and unsafe masks remain reviewable instead of being fabricated.
