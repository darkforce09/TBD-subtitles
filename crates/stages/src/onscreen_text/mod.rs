//! Visible Japanese recognition and presentation, separate from spoken dialogue.
//!
//! **Role:** detect, read, track, translate, join, review and typeset visible writing.
//! **Position:** stage logic below the pipeline and above media and model backends.
//! **Signals and state:** bounded frame streams, representative crops and typed text documents.
//! **Invariants:** source pixels are read-only; uncertain surfaces receive nearby translations.

pub mod detect;
mod event_buffer;
mod furigana;
pub mod geometry;
mod glyphs;
pub mod read;
pub mod reference;
pub mod replace;
pub mod review;
pub mod track;
pub mod translate;
pub mod typeset;
pub mod unify;

pub type TextResult<T> = Result<T, inference::ocr::OcrError>;
