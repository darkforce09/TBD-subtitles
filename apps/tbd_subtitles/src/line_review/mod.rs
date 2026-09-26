//! Line review: for each flagged line, play its clip, compare the engines' hypotheses, pick or
//! type the text, and save it for the review step to time again.

pub(crate) mod events;
pub(crate) mod models;
pub(crate) mod services;
pub(crate) mod ui;
