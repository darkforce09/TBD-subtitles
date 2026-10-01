# Frame queue

A bounded queue between a frame decoder and the code that reads its frames, and the recycled
buffers frames are read into.

## Contents

```text
crates/media_io/src/frame_queue/
├── mod.rs   `Producer` and `FrameQueue`: a producer on its own thread, at most `depth` items ahead
├── pool.rs  `BufferPool` and `PooledBuffer`: same-size buffers that return to the pool when dropped
└── tests/   order, early stop, error delivery and buffer reuse
```

## How it works

`FrameQueue::spawn` runs a `Producer`, such as an FFmpeg frame stream, on a thread of its own and
sends its items into a `sync_channel` of `depth` places. The decoder therefore keeps decoding while
the reader screens, blends or encodes, and stops when the queue is full, so memory stays bounded.
`recv` hands the items out in order; `finish` drops the receiver, which ends a producer the reader
left early, joins the thread and returns how the producer ended. A frame's samples live in a
`PooledBuffer` taken from a `BufferPool`; dropping the frame puts the buffer back, so a stream of
frames allocates only until the pool holds as many buffers as are in flight.

## Boundaries

- Depends on: the standard library only.
- Used by: `yuv::YuvFrame`, the frame streams of `video_frames`, and the detection scan, the
  region source and the localized video in `crates/stages/`.
- Rules: the producer's `finish` runs exactly once; an error or a panic on the producer thread
  comes back to the reader, never lost.
