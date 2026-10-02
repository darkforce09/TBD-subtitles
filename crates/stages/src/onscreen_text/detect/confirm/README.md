# Keyframe confirmation

The scan's last phase: the keyframes of the persistent, confident occurrences are confirmed once
by the server detector, for every occurrence that shares them, and the confirmed ones get their
crop and keyframe image.

## Contents

```text
crates/stages/src/onscreen_text/detect/confirm/
├── mod.rs  `Closed`, `confirm_keyframes`: keyframes in a fixed order, from memory or stills, a chunk at a time
└── tests/  confirmation order, chunking, prefetched stills, dropped occurrences and identical files
```

## How it works

The crop and keyframe folders are emptied first. The distinct keyframes of the occurrences with
at least `min_confirm_frames` frames and a first-sample score of at least
`min_confirm_confidence` (five and 0.55 in production) are taken in the order occurrences first
need them, eight at a time; an occurrence whose keyframe no such occurrence shares is never
confirmed and leaves the document. For each chunk, a keyframe the scan still holds is
converted from its yuv420p frame; the others come from `FrameSource::stills`, decoded for the next
chunk on a thread while the current chunk is confirmed. Every picture is padded as the screening
pictures are and sent in one `TextScreening::confirm` call, which answers in the order asked. For
each keyframe, every occurrence using it takes the best overlapping full-resolution region through
`crops::confirm`; its crop, and the keyframe image once, go to the PNG writer thread. The writer
is joined before the document of confirmed occurrences comes back.

## Boundaries

- Depends on: the sibling `crops`, `screen`, `source`, `timing` and `writer` modules,
  `inference::ocr::pool` and `media_io::yuv`.
- Used by: the scan coordinator, after the last group.
- Rules: the order depends only on the document; each distinct keyframe is confirmed once; a
  keyframe from memory and the same still from the video give the same document and files
  (`tests/confirm.rs`).
