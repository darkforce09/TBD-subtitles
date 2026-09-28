//! Why a finished job does not pass the quality check, in the owner's words, each with its fix.
//!
//! **Role:** name each pass rule a job can break (layout, speech with no subtitle, the aligner's
//! offset, a failed language-model call, reading speed) with a plain title, what to do about it,
//! and the button that does it, when there is one.
//!
//! **Position:** built by `services::line_counts::problems` from the quality check; drawn by the
//! file card; counted on the sidebar row.
//!
//! **Signals and state:** none; plain data.
//!
//! **Invariants:** one problem per broken pass rule, so a job has none exactly when it passes
//! (`QcReport::passes`).

/// One broken pass rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Problem {
    /// This many findings that break a layout rule outright.
    Layout(usize),
    /// Heard speech left with no subtitle, the first stretch starting at this video second.
    UncoveredSpeech(f64),
    /// The aligner's timing is off by 30 ms or more.
    Offset,
    /// This many language-model calls failed.
    FailedCall(usize),
    /// Only this share of subtitles is within 20 characters per second, under the target.
    ReadingSpeed(f64),
}

/// What a problem's button does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Remedy {
    /// Open Check Lines on every line at the one nearest this video second, to find what was
    /// left out.
    ShowNearbyLines(f64),
    /// Run the language-model steps again, and the steps after them.
    TryAgain,
    /// Open Check Lines on the lines too fast to read.
    ShowTooFastLines,
}

impl Problem {
    /// The problem in a few words.
    pub(crate) fn title(self) -> String {
        match self {
            Problem::Layout(1) => "1 subtitle breaks a layout rule".to_string(),
            Problem::Layout(n) => format!("{n} subtitles break a layout rule"),
            Problem::UncoveredSpeech(_) => "Speech with no subtitle".to_string(),
            Problem::Offset => "The aligner's timing is off by 30 ms or more".to_string(),
            Problem::FailedCall(1) => "1 language-model call failed".to_string(),
            Problem::FailedCall(n) => format!("{n} language-model calls failed"),
            Problem::ReadingSpeed(share) => {
                format!("Only {:.1} % of subtitles are easy to read", share * 100.0)
            }
        }
    }

    /// The problem in a few words once it is gone, as the Fix It result lists it.
    pub(crate) fn cleared_title(self) -> String {
        match self {
            Problem::Layout(1) => "1 subtitle broke a layout rule".to_string(),
            Problem::Layout(n) => format!("{n} subtitles broke a layout rule"),
            Problem::UncoveredSpeech(_) => "Speech with no subtitle".to_string(),
            Problem::Offset => "The aligner's timing was off by 30 ms or more".to_string(),
            Problem::FailedCall(1) => "1 language-model call failed".to_string(),
            Problem::FailedCall(n) => format!("{n} language-model calls failed"),
            Problem::ReadingSpeed(share) => {
                format!("Only {:.1} % of subtitles were easy to read", share * 100.0)
            }
        }
    }

    /// What to do about it.
    pub(crate) fn fix(self) -> &'static str {
        match self {
            Problem::Layout(_) => {
                "Open them in Check Lines; editing a line rebuilds its subtitles."
            }
            Problem::UncoveredSpeech(_) => "Show the nearby lines to find what was left out.",
            Problem::Offset => {
                "Every subtitle may sit slightly early or late. There is no per-line fix."
            }
            Problem::FailedCall(_) => {
                "Some lines may be missing. Try Again runs that step and the ones after it."
            }
            Problem::ReadingSpeed(_) => {
                "The target is 95 % within 20 characters per second. Shorten some of the Too \
                 fast lines."
            }
        }
    }

    /// The button beside it, if there is one.
    pub(crate) fn remedy(self) -> Option<Remedy> {
        match self {
            Problem::UncoveredSpeech(at) => Some(Remedy::ShowNearbyLines(at)),
            Problem::FailedCall(_) => Some(Remedy::TryAgain),
            Problem::ReadingSpeed(_) => Some(Remedy::ShowTooFastLines),
            Problem::Layout(_) | Problem::Offset => None,
        }
    }
}

impl Remedy {
    /// The button's label.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Remedy::ShowNearbyLines(_) => "Show Nearby Lines",
            Remedy::TryAgain => "Try Again",
            Remedy::ShowTooFastLines => "Show Lines",
        }
    }
}

#[cfg(test)]
#[path = "tests/problem.rs"]
mod tests;
