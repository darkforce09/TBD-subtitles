//! What the output step left: `outputs/output`, the subtitle file beside the video and what it
//! replaced or moved aside.

use serde::{Deserialize, Deserializer, Serialize};

/// The output step's record.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct OutputRecord {
    /// The subtitle file beside the video.
    pub path: String,
    /// The file already held exactly this text, so nothing was written.
    #[serde(deserialize_with = "bool_or_text")]
    pub unchanged: bool,
    /// Where a different file at `path` was copied before it was replaced.
    #[serde(default)]
    pub backup: Option<String>,
    /// Where the job's file of another format went when the output format changed.
    #[serde(default)]
    pub retired: Option<String>,
    /// The subtitle file beside the localized video, when the job writes one.
    #[serde(default)]
    pub localized: Option<String>,
}

/// `true`, or the text `"true"` that records written as plain notes hold.
fn bool_or_text<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flag {
        Bool(bool),
        Text(String),
    }
    Ok(match Flag::deserialize(deserializer)? {
        Flag::Bool(b) => b,
        Flag::Text(t) => t == "true",
    })
}
