# Text detection

The first visual stage. It screens the video at full resolution for visible writing, finds each
region's exact first and last frame, and confirms every occurrence once on its keyframe, which
yields its crop and its keyframe image.

## Contents

```text
crates/stages/src/onscreen_text/detect/
├── confirm/    confirmation after the scan: keyframes from memory or decoded stills, in a fixed order
├── crops.rs    rectified colour and grey crops, plain-surface checks and one occurrence's confirmation
├── mod.rs      `scan`, `scan_measured` and the scan's limits
├── probe/      bisection probes: transitions, probe screening ahead of the queue, presence on a frame
├── regions.rs  fixed-anchor signatures, mutually unique matches and tiled occurrence frames
├── scan/       the in-order coordinator: decoding, screening ahead, jobs in flight, region following
├── screen.rs   the sample schedule, luma repeats and the padded picture a screened frame is sent as
├── source.rs   the frame source contract and the FFmpeg source: yuv420p frames ahead, decoded stills
├── tests/      association, crops, sampling, timing and FFmpeg source checks, the scripted video and sessions
├── timing.rs   `ScanStats`: the scan's phases, counts and the sessions' start-up and notes
├── window/     held frames: samples with their gaps in groups, and the keyframe candidates
└── writer/     the PNG thread that writes crops and keyframe images during confirmation
```

## How it works

```text
FfmpegSource ── yuv420p frames, up to 45 s ahead ──▶ scan coordinator ──▶ ScreenJob (Screen) ──▶ sessions
                                                │   ▲                                          │
                                   groups wait  │   └──── ScreenResult, any order ◀────────────┘
                                                ▼
                     tracker: observe in sample order ─▶ probes (Probe priority) ─▶ settle
                                                │
                     keyframe candidates (≤ 16 GiB) ─▶ confirmation ─▶ PNG writer thread
```

`FfmpegSource` decodes every frame at the video's own size as yuv420p, exactly as encoded, on a
thread of its own through a `FrameQueue` up to forty-five seconds deep (up to 3.5 GiB), and reads
full-resolution stills by frame index through accurate seeks, eight at a time. The scan treats
every `k`-th frame as a sample, where `k` is half the frame rate rounded, together with both
frames around every shot cut and the final frame, so the frames between two samples never cross a cut.
A sample whose luma, in 32 by 32 blocks, stays within a mean of 4 levels of the last screened
sample repeats that sample's regions; every other sample is converted in the stream's own matrix
and range to rgb24, padded below with black rows to a multiple of 32, and joins the group being
gathered with the frames since the previous sample.

A group closes when its pictures fill the sessions' batch, or when it holds twice a batch of
samples; its pictures go to the sessions as one screening job at once, and decoding goes on while
at most six groups per session wait. Results come back in any order and are kept by sequence
number; the groups are observed strictly in order, each only once its job is answered, so one or
two sessions and any order of answers give the same document. Regions are followed from sample to
sample: a match needs more than 0.45 box overlap and an unchanged picture at the region's fixed
anchor box, unique in both directions, and a cut clears every match. The signatures read the
anchor box from the luma plane as full-range grey, a perspective quad rectified in grey, and the
regions are compared side by side. A new region entered somewhere after the previous sample, and
an unmatched one left somewhere before this one. Bisection over the group's held frames finds the
first present or first absent frame in at most ceil(log2(k)) probes; all searches of a group
advance together, each step's probes converted from the held frames and screened as probe jobs
that the sessions run ahead of the waiting screening jobs. A group's bisection ends before the
next group is observed, and its frames are then released. An occurrence keeps its entry frame and
one frame per matched sample; each frame ends where the next begins and the last ends at the first
absent frame, so the timing is exact. Quads are in source pixels.

Each occurrence's keyframe is the sample nearest the middle of its interval. While it is active,
the keyframe candidates hold, shared and counted once, every sample from the last one at or before
the middle of its start and its latest sample; when it ends, only the chosen sample stays. Past a
budget of 16 GiB the active window holding the most samples gives up its frames, and that
occurrence's keyframe is decoded from the video instead. After the stream, confirmation takes the
distinct keyframes in the order occurrences first need them, eight at a time: held ones are
converted from memory, the others decoded as stills while the previous chunk is confirmed. The
server detector confirms each keyframe once, and the best overlapping region replaces that frame's
quad; an occurrence the server detector does not confirm is dropped as screening noise, as is one
shorter than 0.15 s or wider than half the frame. The keyframe gives the occurrence's surface
colour for all its frames, its rectified crop in `visual/crops/` and its keyframe image, at most
1280 pixels wide, in `visual/keyframes/`, both written by one PNG thread that the scan joins
before it returns. Both folders are emptied when confirmation starts, so a rerun never leaves
stale files behind. At most one million observations and one hundred thousand occurrences are
held; beyond that the scan fails and asks for shorter jobs. Progress is reported once per 24
decoded frames and at the last frame, then once per confirmed keyframe.

`scan_measured` returns the document with `ScanStats`: the time waiting on the decoder, converting
samples, waiting for screening results, probing, comparing signatures, confirming, and converting
or decoding keyframes, the frames decoded, screened and probed, the keyframes from memory and from
the video, the most memory the candidates held, and the sessions' warm-up, engine build and notes.

## Public surface

- `scan(source, stream, cuts, root, pool, progress)` and `scan_measured(…)`, the latter returning
  `(TextDocument, ScanStats)`; `pool` is any `inference::ocr::pool::TextScreening`.
- `FrameSource`, the frames a scan reads, and `FfmpegSource::open(programs, video, stream)`.
- `ScanStats` and `ScanStats::notes`, the step notes: `decode_wait_s`, `convert_s`, `screen_s`,
  `probe_s`, `signature_s`, `confirm_s`, `stills_ram_s`, `stills_ffmpeg_s`, `warmup_s`,
  `engine_build_s`, `frames_decoded`, `frames_screened`, `frames_probed`, `keyframes_ram`,
  `keyframes_ffmpeg`, `keyframes_held_peak_mib`, and each session note as `detector_<key>`.
- `crop`, a quad's rectified lettering plane.

## Boundaries

- Depends on: `media_io` (the yuv420p frame stream, `frame_queue`, `yuv` conversion, luma
  thumbnails and grey crops, stills), `inference::ocr::pool` (the `TextScreening` contract),
  `job_model` contracts, `rayon`, and the sibling `geometry` and `png` modules.
- Used by: `pipeline::tasks::onscreen` for the measured scan; `replace::verify` and
  `tools/visual_validation` for `crop`.
- Rules:
  - source videos are only read; no full-video image extraction; held frames are bounded by the
    waiting groups, the decode queue and the candidates' budget;
  - results are applied in sample order, so one and two sessions and any answer order give the
    same document and files (`one_and_two_sessions_give_identical_documents_and_files` and
    `results_answered_out_of_order_are_applied_in_sample_order` in `scan/tests/scan.rs`);
  - a keyframe from memory and from the video give the same document
    (`keyframes_beyond_the_candidate_budget_come_from_the_video_with_the_same_document`);
  - limits fail explicitly; every confirmed occurrence gets exactly one crop and one keyframe
    image, written before the scan returns.

## Related documentation

- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — detection, review
  and presentation of visible writing.
- [Pipeline](/documentation/architecture/pipeline.md) — step order, workers and resume.
- [Detector pool contract](/crates/inference/src/ocr/pool/README.md) — the screening and
  confirmation jobs the scan sends.
