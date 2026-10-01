//! The contract of one approved sign in the library shared by every episode.
//!
//! **Role:** carry what a later occurrence of the same writing starts from: the English, its
//! confidence and lettering style, and the patch and mask of the replacement that was approved.
//! **Position:** bottom-layer data; `pipeline::library` stores it in `library.redb`, keyed by the
//! normalised Japanese and the crop hash it repeats here.
//! **Signals and state:** serializable values; no I/O.
//! **Invariants:** `episodes` names the job a sign came from first, then every job that recorded
//! it again; the English is the English the approved replacement letters, never invented.

use serde::{Deserialize, Serialize};

use super::LetteringStyle;

/// One approved sign: an occurrence whose replacement read back cleanly and that no owner
/// correction rejected.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct LibrarySign {
    /// The Japanese as the approved occurrence read it.
    pub japanese: String,
    /// The 64-bit difference hash of the approved occurrence's keyframe crop.
    pub crop_hash: u64,
    pub english: String,
    /// The approved occurrence's confidence; 1 when the owner saved it in Check Text.
    pub confidence: f64,
    pub style: LetteringStyle,
    /// The lettering patch (RGBA PNG) of the approved replacement's first plate.
    pub patch_png: Vec<u8>,
    /// The erase mask (8-bit PNG) of the approved replacement's first plate.
    pub mask_png: Vec<u8>,
    /// The jobs that recorded the sign: the job it came from first, then every later one.
    pub episodes: Vec<String>,
    /// When the sign was first recorded, in seconds since the Unix epoch.
    pub added_s: u64,
}

impl LibrarySign {
    /// The job the sign came from.
    pub fn origin(&self) -> Option<&str> {
        self.episodes.first().map(String::as_str)
    }
}
