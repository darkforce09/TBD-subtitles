//! Review visible Japanese translations alongside the dialogue review.
//!
//! **Role:** expose text occurrences, corrections and actual ASS previews.
//! **Position:** desktop feature below the application composition layer.
//! **Signals and state:** loaded job artifacts and asynchronous preview frames.
//! **Invariants:** the feature never modifies source video or dialogue corrections.

pub(crate) mod models;
pub(crate) mod services;
pub(crate) mod ui;
