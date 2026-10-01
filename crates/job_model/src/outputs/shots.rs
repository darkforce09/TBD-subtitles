//! The shot-change times of a video, as kept in `outputs/shot_scan`.

use serde::{Deserialize, Serialize};

/// Every scene change the scan reported, ascending by time.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ShotChanges {
    pub cuts: Vec<ShotCut>,
}

/// One reported scene change and how strongly the picture changed there.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ShotCut {
    pub time_s: f64,
    /// FFmpeg's scdet score, 0 to 100; a hard cut scores high, a flash or fast pan lower.
    pub score: f64,
}

impl ShotChanges {
    /// The times of the cuts scoring at least `min_score`.
    pub fn times_at_least(&self, min_score: f64) -> Vec<f64> {
        self.cuts
            .iter()
            .filter(|c| c.score >= min_score)
            .map(|c| c.time_s)
            .collect()
    }
}
