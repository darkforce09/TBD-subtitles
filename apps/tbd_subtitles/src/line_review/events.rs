//! What the review view asks the application to do.

use crate::line_review::models::clip::Sound;
use crate::line_review::models::session::LineList;

/// One request from the review view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewEvent {
    /// Edit this line.
    Open(String),
    /// Put the reading with this tag in the open line's text.
    Pick(String),
    /// The open line's text, as the owner typed it.
    EditText(String),
    /// The open line's flags.
    SetFlags(Vec<String>),
    /// Throw away the open line's edit.
    Discard,
    /// Save the open line's edit as a correction and time it again.
    Save,
    /// Keep the open line as the language model has it, and time it again.
    LooksRight,
    /// Take back this line's correction.
    Revert(String),
    /// Edit the next, or the previous, line shown.
    Step {
        forward: bool,
    },
    /// Show these lines.
    List(LineList),
    /// The search field's text.
    Search(String),
    /// Show the lines of every group again.
    ClearGroup,
    Play(Sound),
    Stop,
    /// Leave the review for the job's report: the Overview tab, or another job selected.
    Close,
}
