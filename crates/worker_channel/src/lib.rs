//! The channel between a step's worker process and the job runner: framed binary messages on
//! pipes, never text lines.
//!
//! **Role:** the one definition of what crosses the pipes: [`frame`] the frame codec (a tag byte,
//! a little-endian `u32` length, the payload), [`address`] the table and key an input or output
//! value belongs to, [`progress`] the payload of a progress frame, and [`worker`] the worker's
//! side: its private frame descriptor and the calls that send each kind of frame.
//!
//! **Position:** layer 0, beside `job_model` and `child_process`; the job runner in `pipeline`
//! reads frames with it and all three app binaries send them through it. It depends on no
//! workspace crate.
//!
//! **Signals and state:** [`worker::install`] rewires the worker process's descriptor 1 once;
//! everything else is pure encoding over any `Read` or `Write`.
//!
//! **Invariants:** a frame is written whole or not at all as far as another thread can see (one
//! lock per sink); a reader treats a clean end before a header as the end of the stream and any
//! other cut as an error; payloads are bytes, never decoded as text by the codec.

pub mod address;
pub mod frame;
pub mod progress;
pub mod worker;
