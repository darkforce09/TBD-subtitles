**Status:** live

# Visual tracking and video acceleration

How the on-screen text pipeline achieves sub-pixel perspective stability, flicker-free
plate inpainting, and an accelerated localized video encode using binary frame tables in `redb`.

## The challenges of video-rate replacement

In-place on-screen text replacement currently faces two major throughput and stability bottlenecks:

1. **Monolithic full-video re-encoding:** `localized_video` decodes, blends, and encodes all
   44,489 frames of a 30-minute episode with `hevc_nvenc`, taking 265 seconds (over 50% of the
   visual processing budget), despite only 15 to 20 seconds of the video containing active text patches.
2. **Per-frame independent inpainting:** inpainting moving signs across dozens of consecutive frames
   with LaMa produces minor frame-to-frame textural variations (flicker) and wastes neural compute.

With per-frame binary records indexed in `job.redb`, the pipeline transitions from discrete
keyframes to continuous temporal video processing.

## 1. Planar homography and optical flow tracking

When a camera pans across writing or a sign tilts in perspective, a static bounding box slides
or jitters relative to the underlying surface.

### The solution

- A lightweight optical flow tracker or planar homography estimator computes a 3×3 perspective
  transformation matrix ($H_t$) for each active frame of an occurrence.
- The 9 floating-point values (36 bytes archived in `rkyv`) are stored in the `frames` table of
  `job.redb` keyed by `(occurrence_id, frame_index)`.
- During compositing, `tiny-skia` applies the homography matrix directly to the rendered English
  vector lettering.
- The translated text locks to the 3D surface plane, following camera motion, zoom, and tilt without wobble.

## 2. Temporal inpaint warping (flicker-free plate reuse)

Running LaMa independently on 60 frames of a single sign is slow and introduces high-frequency
background texture jitter.

### The solution

- **Keyframe inpaint:** LaMa executes only on the primary clean keyframe of an occurrence.
- **Motion-compensated warping:** for subsequent frames within the same camera shot, the clean
  inpainted background plate is warped along the surface homography vectors.
- If tracking confidence drops below threshold (e.g. sudden occlusion or scene cut), a secondary
  inpaint plate is generated and blended across the transition.
- **Benefits:**
  - Background textures remain completely solid across time with zero neural flicker.
  - LaMa neural network invocations drop by up to 80% per episode.

## 3. Smart lossless segment re-encoding

In Dressrosa 11, only 15 occurrences (approximately 20 seconds of footage out of 30 minutes)
require text alteration. The remaining 98.9% of frames pass through the decoder and encoder unchanged.

### The solution

The pipeline implements GOP-aligned smart rendering:

1. **Identify modified intervals:** query `job.redb` for the exact frame spans containing active
   composed patches.
2. **GOP alignment:** expand each interval outward to the nearest closed keyframe (GOP boundary)
   in the source video.
3. **Targeted re-encode:** FFmpeg decodes, blends patches, and re-encodes only those isolated
   5-to-10 second segments using `hevc_nvenc`.
4. **Lossless concatenation:** unmodified source intervals are stream-copied (`-c copy`) without
   re-compression, and stitched with the re-encoded segments using the Matroska concat demuxer.
5. **Runtime reduction:** `localized_video` runtime drops from **4.5 minutes down to 10–15 seconds**,
   preserving 100% original video quality on untouched scenes.

## 4. Trajectory smoothing and shot-boundary snapping

Text detection confidence fluctuates near entrance and exit boundaries during scene fades or fast
action, occasionally generating 1-frame dropouts or detached 2-frame initial fragments.

### The solution

- A 1D Kalman filter runs over the bounding box coordinate series stored in `job.redb`.
- Brief 1-to-2 frame dropouts are interpolated smoothly.
- The entrance and exit boundaries of an occurrence snap automatically to the nearest shot cut
  from the shot scan (`outputs/shot_scan`), preventing replacement patches from lingering into an
  unrelated cut.

## 5. Multi-modal audio-visual synchronization

Title cards, character introductions, and location banners in anime routinely align with distinct
percussive sound effects or narrator speech onsets.

### The solution

- The visual pipeline cross-references `outputs/text_detect` entries with `outputs/sound_events`
  and speech word starts from `outputs/alignment`.
- When an occurrence entrance lands within 3 frames of a verified sound event or narrator dialogue
  start, its start frame snaps to that exact acoustic timestamp.
- Subtitle appearances achieve director-intended cinematic synchrony.

## Boundaries

- Depends on: [`video_inpainting_pipeline.md`](/documentation/architecture/video_inpainting_pipeline.md),
  [`media_io`](/crates/media_io/README.md), and
  [`job.redb`](/documentation/architecture/binary_storage_plan.md).
- Used by: roadmap milestone M8, `stages::localize`, and `pipeline::tasks::localized`.
- Rules: source videos remain strictly read-only; unchanged frames retain their original bitstream.

## Related documentation

- [Roadmap](/documentation/roadmap.md) — milestone M8.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — baseline steps.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md) — per-frame tables.
