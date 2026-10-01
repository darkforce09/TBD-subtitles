//! Sound events found in the stems, as kept in `outputs/sound_events`: candidates for sound cues and
//! the music and singing stretches.

use serde::{Deserialize, Serialize};

/// One stretch where a sound class scored over its threshold.
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
pub struct SoundEvent {
    /// The AudioSet class name, such as `Explosion` or `Laughter`.
    pub label: String,
    /// The stem it was heard on: `background` or `vocals`.
    pub stem: String,
    pub start_s: f64,
    pub end_s: f64,
    /// The highest smoothed probability inside the stretch.
    pub peak: f32,
}
