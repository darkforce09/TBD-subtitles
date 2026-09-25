//! FFmpeg decoding audio to 16 kHz mono or 44.1 kHz stereo 32-bit float through a pipe, read in
//! fixed-size chunks through a bounded channel so a whole track is never held in memory.
