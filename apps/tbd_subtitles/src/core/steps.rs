//! The pipeline's steps as the window names them: nine stages the owner recognises, and a plain
//! title for every step.
//!
//! **Role:** group the twenty-nine steps into the nine stages the window shows, each with its title
//! ("Settle the words") and what it does while it runs ("Settling the words"), and give every
//! step a plain title in place of its `snake_case` name.
//!
//! **Position:** used by the features' views and status text; depends on `job_model::StepName`.
//!
//! **Signals and state:** none; constant data.
//!
//! **Invariants:** every step belongs to exactly one stage, and the stages' steps, read in
//! order, are `StepName::ALL`.

use job_model::StepName;

/// A group of steps the window shows as one.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Stage {
    /// The stage as a heading: "Settle the words".
    pub(crate) title: &'static str,
    /// The stage while it runs: "Settling the words".
    pub(crate) doing: &'static str,
    /// Its steps, in run order.
    pub(crate) steps: &'static [StepName],
}

/// The nine stages, in run order.
pub(crate) const STAGES: [Stage; 9] = [
    Stage {
        title: "Read the video",
        doing: "Reading the video",
        steps: &[StepName::ProbeDecode, StepName::ShotScan],
    },
    Stage {
        title: "Separate the voices",
        doing: "Separating the voices",
        steps: &[StepName::Separation, StepName::Vad],
    },
    Stage {
        title: "Hear the speech",
        doing: "Hearing the speech",
        steps: &[
            StepName::AsrParakeet,
            StepName::AsrWhisper,
            StepName::DiffSheet,
            StepName::SoundEvents,
        ],
    },
    Stage {
        title: "Settle the words",
        doing: "Settling the words",
        steps: &[
            StepName::Adjudicate,
            StepName::RedecodeParakeet,
            StepName::RedecodeWhisper,
            StepName::Readjudicate,
            StepName::SoundCues,
        ],
    },
    Stage {
        title: "Time the words",
        doing: "Timing the words",
        steps: &[StepName::Alignment, StepName::Review],
    },
    Stage {
        title: "Lay out the subtitles",
        doing: "Laying out the subtitles",
        steps: &[StepName::Cues],
    },
    Stage {
        title: "Translate on-screen text",
        doing: "Translating on-screen text",
        steps: &[
            StepName::TextDetect,
            StepName::TextRead,
            StepName::TextTrack,
            StepName::TextTranslate,
            StepName::TextReview,
            StepName::TextMask,
            StepName::TextInpaint,
            StepName::TextCompose,
            StepName::TextVerify,
            StepName::TextTypeset,
        ],
    },
    Stage {
        title: "Write the subtitles",
        doing: "Writing the subtitles",
        steps: &[StepName::Qc, StepName::Output],
    },
    Stage {
        title: "Write the localized video",
        doing: "Writing the localized video",
        steps: &[StepName::LocalizedVideo],
    },
];

/// The stage `step` belongs to.
pub(crate) fn stage_of(step: StepName) -> &'static Stage {
    STAGES
        .iter()
        .find(|stage| stage.steps.contains(&step))
        .unwrap_or(&STAGES[STAGES.len() - 1])
}

/// The step's plain title, as the window shows it.
pub(crate) fn step_title(step: StepName) -> &'static str {
    match step {
        StepName::ProbeDecode => "Read the video's details",
        StepName::ShotScan => "Find shot changes",
        StepName::Separation => "Separate voices from music",
        StepName::Vad => "Find where people speak",
        StepName::AsrParakeet => "Listen with Parakeet",
        StepName::AsrWhisper => "Listen with Whisper",
        StepName::DiffSheet => "Compare what both heard",
        StepName::SoundEvents => "Detect sounds",
        StepName::Adjudicate => "Language model settles the words",
        StepName::RedecodeParakeet => "Listen again to unsure lines (Parakeet)",
        StepName::RedecodeWhisper => "Listen again to unsure lines (Whisper)",
        StepName::Readjudicate => "Second look at unsure lines",
        StepName::SoundCues => "Write sound cues",
        StepName::Alignment => "Time each word",
        StepName::Review => "Apply your corrections",
        StepName::Cues => "Lay out the subtitles",
        StepName::TextDetect => "Find on-screen writing",
        StepName::TextRead => "Read the Japanese text",
        StepName::TextTrack => "Track writing through the scene",
        StepName::TextTranslate => "Translate writing into English",
        StepName::TextReview => "Apply your text corrections",
        StepName::TextMask => "Find the strokes to erase",
        StepName::TextInpaint => "Paint over the Japanese",
        StepName::TextCompose => "Letter the English in place",
        StepName::TextVerify => "Check the lettered English",
        StepName::TextTypeset => "Place and style the English text",
        StepName::Qc => "Quality check",
        StepName::Output => "Save the subtitle file",
        StepName::LocalizedVideo => "Save the localized video",
    }
}

#[cfg(test)]
#[path = "tests/steps.rs"]
mod tests;
