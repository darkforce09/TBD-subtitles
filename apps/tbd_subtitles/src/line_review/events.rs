//! What the review view asks the application to do.

use crate::line_review::models::clip::Sound;

/// One request from the review view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewEvent {
    /// Edit this line.
    Open(String),
    /// Put the reading with this tag in the edited text.
    Pick(String),
    /// The edited text, as the owner typed it.
    EditText(String),
    /// The edited line's flags.
    SetFlags(Vec<String>),
    /// Save the edited line as a correction and time it again.
    Save,
    /// Take back this line's correction.
    Revert(String),
    /// Edit the next, or the previous, line shown.
    Step {
        forward: bool,
    },
    /// Show every line, or only the flagged and corrected ones.
    ShowAll(bool),
    Play(Sound),
    Stop,
    /// Leave the review for the job's report.
    Close,
}
