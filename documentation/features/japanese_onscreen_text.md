**Status:** live

# Japanese on-screen text

The visual translation pipeline and Check Text interface are implemented; M4 acceptance is still
underway. A video job produces one ASS file containing dialogue, sound cues and English for
visible Japanese writing. The source video stays unchanged. Pilot coverage, full-episode quality,
resource limits, packaged playback and owner acceptance are not yet established by this document.

The owner accepts missed faint text and false detections as current limitations. The AppImage
is built and passes its host startup smoke check. Detection screens sampled proxy frames and
bisects the exact boundaries, and Claude reads each text event once from its keyframe; the
single-episode benchmark that measures this, full GUI correction acceptance and VLC playback
remain open.

## Where it lives

- The six visual stages: [on-screen text stages](/crates/stages/src/onscreen_text/), orchestrated
  by [visual pipeline tasks](/crates/pipeline/src/tasks/onscreen.rs).
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
- **Claude fallback**, enabled by default. The installed, signed-in Claude CLI reads and
  translates every detected text event once from its keyframe, under the existing shared call
  limit with structured replies; local OCR and Qwen answer whatever it leaves. Off, or with the
  CLI unavailable, the local models supply the result and compatible reference wording can
  improve it. There is no paid API backend or automatic billing fallback.
- **Reference subtitles**, an editable folder path. The reference loader reads ASS files directly
  in that folder; it does not recursively scan the media library. No path means no references.
- **Model availability**, with Open Models and Download Missing. Missing models prevent a job
  from starting and are listed in the existing model banner. Claude readiness links to This
  Computer; a missing or unsigned-in CLI leaves an actionable warning on uncertain text.

Changes save as they are made. They apply to jobs that have not started. Saved job records without
visual settings keep visual processing disabled; opening or resuming them does not silently add a
new scan. Run Again with Current Settings explicitly adopts the enabled setting for that video.

### One combined job

After dialogue cue construction, the job runs six resumable visual steps before final quality
checking and output:

```text
Dialogue cues -> Detect -> Read -> Track -> Translate -> Review -> Typeset -> QC -> combined ASS
```

| Step | Work and retained result |
|---|---|
| Detect | Streams a 640-wide proxy of every frame through FFmpeg with packet presentation timestamps. Screens every `round(fps / 2)`-th frame plus the first and last frame of each shot with PP-OCRv5, and bisects the frames between two samples to the exact frame where writing appears or vanishes. Keeps one observed frame per sample or boundary, a keyframe still nearest the midpoint of each occurrence, and a full-resolution, perspective-corrected crop from that keyframe; cuts or changed writing start new occurrences. |
| Read | Reads Japanese with PP-OCRv5 through oar-ocr, using manga-ocr for difficult crops. Consolidates compatible adjacent readings and groups conservative furigana evidence; uncertain readings remain flagged. |
| Track | Checks that every sampled quad stays within tolerance of the keyframe quad without decoding video. A moving or unverified surface gains a review warning and nearby placement. |
| Translate | Asks Claude first, one call per keyframe frame with the whole-frame still and the crops of its regions, for each region's Japanese, English, confidence and box plus any other writing on the frame. Qwen3.5-4B, with short dialogue context and the glossary, loads only for occurrences Claude leaves unanswered; compatible corrected occurrences consolidate before review. |
| Review | Checks a saved correction's source identity before applying the owner's English, timing and presentation independently of dialogue corrections. |
| Typeset | Produces ASS text or vector glyph events, with stable typography and frame-specific geometry. Unsafe replacement uses nearby English with a review warning. |

Claude image requests run as many at once as the job's `claude` process count, before any local
model loads; requests for the same keyframe share a cache lock, and the existing Claude process
cap, cancellation and call logging apply. Every hinted region must come back exactly once, or its
occurrences fall to the local phase; a region whose box lands away from its hint is kept with a
review warning; other writing Claude reports on the frame becomes a new occurrence with the
surrounding event's timing, nearby placement and a review warning. Qwen runs through mistral.rs in
`tbd-subtitles-llm`, separate from the ONNX Runtime processes, and only for occurrences without
a valid Claude answer. Readings without Japanese script, or with OCR confidence below 0.5, bypass
Qwen and remain unresolved. The shared GPU lock prevents simultaneous GPU stages. The model store
downloads pinned exported models; there is no local model conversion.

Detection samples the proxy stream at two frames per second and at every shot boundary, so
writing visible for fewer frames than the sample step that falls between two samples and touches
no cut is missed. Near-duplicate samples reuse the previous detections; the rest are screened
eight at a time. Detection compares each candidate with the occurrence's fixed first signature,
rather than allowing small changes to accumulate against successive samples. Matching requires a
unique association in both directions. The signature allows small alignment jitter but checks
individual pixel differences, 8 by 8 cells and the whole crop, including its edges; a changed
glyph in a long line or newly visible scrolling text can therefore split the occurrence, and the
bisection finds the exact frame of that change. Perspective correction makes slanted crops
upright for recognition. Surface-colour safety is measured on the full-resolution keyframe, so
that correction cannot turn an unsafe background into permission to cover it.

Adjacent occurrences consolidate only when Japanese readings, geometry and timing agree, with at
most one observed source-frame gap and no known cut or ambiguous match. Once translated, English
must also agree and both confidence scores must be on the same side of the 0.85 display threshold.
This keeps faint fragments separate from a confidently translated interval. A merged interval
retains every measured frame, representative crop, warning and provenance note, with the lowest
confidence of its parts. Small kana above a containing kanji line can become furigana evidence;
after translation, any English words from that small line must already appear in its parent.

A local translation of a reading containing at least four katakana characters and kanji requires
independent image verification: its local confidence is capped at 0.84. This conservative check
helps catch shortened compound names and qualifiers. Without available Claude verification it
stays flagged and unrendered until reviewed; the check does not establish translation completeness.

Frame buffers, thumbnails and preview streams are bounded: the scan holds the frames between at
most eight pending samples, four full-resolution stills and the packet table. The scan fails
explicitly beyond one million geometry observations or one hundred thousand occurrences, and when
the decoded frame count differs from the packet count. Each ASS event buffer has a
128 MiB budget: oversized vector lettering for one occurrence uses a flagged nearby label, while
an oversized combined buffer fails explicitly. These bounds do not guarantee a particular process
memory peak or disk usage; retained crops, observations and caches occupy the job's work directory.

Reference translations are conservative. A nearby dialogue anchor must match the dub scene and
the reference sign wording must agree with the independent translation of the visible reading.
Only wording and its source are reused. Placement, geometry and timing come from the current
video. An absent folder falls back to local translation; a mismatched scene supplies no wording.

### Replacement and fallbacks

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

### Reviewing the result

1. Queue a video normally. Its progress includes the visual steps, their elapsed time and the
   estimate for the combined job. The sidebar keeps one video entry.
2. In Overview, inspect detected, translated, fallback and unresolved text counts, visual
   processing time and the final ASS path. Check Text opens the visual review directly.
3. In Check Text, select an occurrence by thumbnail and timestamp. The view shows the Japanese
   reading, English, confidence, translation provenance and review status; a filter narrows the
   list to flagged occurrences.
4. Compare Original and English previews, play, scrub or step by one source frame. FFmpeg renders
   the actual exported ASS before reducing the picture for the preview. Unsaved editor changes
   do not appear in that exported preview.
5. Edit the English, start/end times, placement, size or replacement/nearby treatment. **Save &
   regenerate ASS** keeps the result and queues a correction run; **Undo** removes that occurrence's
   saved correction. **Retry selected text** requests another reading and translation for the occurrence.
6. Wait for regeneration and inspect the rendered result again. Changes invalidate affected visual
   stages and final output while valid audio work remains reusable. Each **Retry selected text**
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
messages and failures. Completion follows final QC/output after audio and visual processing;
there is no separate scan-complete notification. Replacing a job's SRT with ASS uses the existing
subtitle backup and retirement mechanism.

## Data

All visual files belong to the same [work directory](/documentation/glossary.md#work-directory)
as the audio stages:

| File or contract | Purpose |
|---|---|
| `job.json` | Job settings, model location and stage fingerprints/measurements, including visual processing. |
| `visual/text_detect.json` through `visual/text_typeset.json` | Typed `TextDocument` outputs: readings, tracks, provenance, warnings, source identity, presentation and rendered status, plus review warnings without a current occurrence. |
| `visual/crops/` | Representative full-resolution crops from each keyframe, used by OCR, Claude image requests and review thumbnails. |
| `visual/keyframes/` | One 1280-wide whole-frame still per keyframe frame, sent to Claude with the crops it holds. |
| `visual/readings/` | Cached readings keyed by crop, OCR model pins and retry generation. |
| `visual/translations/` | Cached structured replies keyed by request, model identity/pins and retry generation; a keyframe request also includes the still, its crops and the highest retry generation among its regions. |
| `visual/corrections.json` | Per-occurrence `TextEdit` values with original-source fingerprints and retry requests; written atomically under `visual/corrections.json.lock`. |
| `visual/events.ass` | Typeset visual events merged into the final ASS. |
| `output.json` | The exported subtitle path and any backup or retired output. |

The [visible-text contracts](/crates/job_model/src/onscreen/) keep visual geometry separate from
dialogue layout. Times use the normalized presentation timeline and positions use source pixels;
the typesetter maps them to the ASS script canvas. Model identities, relevant settings, reference
contents and correction inputs participate in resume decisions. Editing an occurrence does not
change the source video or the dialogue correction file.

## Design

The feature uses the approved desktop components: one sidebar row, the existing progress and
Overview cards, Check Text beside Check Lines, and an On-screen Text tab in Settings. It has no
separate translation launcher or queue. Text review uses the same palette, typography, controls
and save-and-regenerate pattern as line review.

The output favours readable, reviewable English when clean replacement cannot be justified.
Restoring the exact hidden background in the supplied board, title-card and name-card examples
is outside ASS-only output; generative frame editing is not part of this implementation.

## Open work

- [M4 — Japanese on-screen text](/documentation/roadmap.md#m4--japanese-on-screen-text): complete
  annotated pilot validation for the supplied categories and Dressrosa 11, 16 and 39; every
  readable occurrence should be translated or explicitly flagged, with missed faint text and
  false detections accepted by the owner. Validate credits, visible lyrics,
  vertical writing, brief appearances, fades, perspective, occlusion, repeated signs, cuts and
  mismatched references. The targets are timing within one source frame and accepted tracking
  error within two pixels at 1080p; failed tracks require a flagged fallback.
- The same milestone requires the complete GUI correction flow, VLC playback, a 20–30 minute episode
  benchmark reporting added visual time and compliance with 8 GB RAM / 5.5 GB worker VRAM,
  packaging and host smoke checks, and explicit owner acceptance of the UI and playback.
  Acceptance remains open; unit tests and the harness do not establish these media-level results.

## Decisions

- Local detection finds and times the writing; Claude reads it. The chosen OCR exports are
  documented by [oar-ocr](https://github.com/GreatV/oar-ocr/blob/main/docs/models.md), and
  difficult Japanese crops use [manga-ocr](https://github.com/kha-white/manga-ocr). Exports are
  pinned in the repository's model manifest and downloaded without conversion.
- Sampled screening with bisected boundaries and one Claude call per keyframe: the
  [decision entry](/documentation/decisions/stack_and_pipeline.md#2026-09-29--on-screen-text-is-found-by-sampled-screening-with-bisected-boundaries-and-read-once-per-event-by-claude-vision).
- [ASS positioning, transforms and vector drawing](https://aegisub.org/docs/latest/ass_tags/)
  keep the source video intact and allow one file to carry dialogue, sound cues and signs.
- References contribute verified wording, never unchecked timing or placement from a different
  edit. Unreadable content and unsafe masks remain reviewable instead of being fabricated.
