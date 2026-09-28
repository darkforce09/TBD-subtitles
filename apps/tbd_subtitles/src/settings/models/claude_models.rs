//! The `claude` models the owner chooses from, for a run's language model and for Fix It.

/// One `claude` model as the Engines tab offers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClaudeModel {
    /// The name `claude --model` takes.
    pub(crate) name: &'static str,
    /// Its name in the window, such as "Claude Opus".
    pub(crate) label: &'static str,
    /// A word after the label in the list, such as "fast"; empty for none.
    pub(crate) tag: &'static str,
    /// What choosing it means, as the help line under the list says.
    pub(crate) how: &'static str,
}

/// The models offered, in the list's order.
pub(crate) const CLAUDE_MODELS: [ClaudeModel; 4] = [
    ClaudeModel {
        name: "sonnet",
        label: "Claude Sonnet",
        tag: "",
        how: "Balanced speed and accuracy.",
    },
    ClaudeModel {
        name: "opus",
        label: "Claude Opus",
        tag: "",
        how: "More careful choices; slower and uses more of your plan.",
    },
    ClaudeModel {
        name: "fable",
        label: "Claude Fable",
        tag: "",
        how: "The most capable model; slowest.",
    },
    ClaudeModel {
        name: "haiku",
        label: "Claude Haiku",
        tag: "fast",
        how: "Fastest and cheapest; more mistakes.",
    },
];

/// The offered model named `name`.
pub(crate) fn find(name: &str) -> Option<&'static ClaudeModel> {
    CLAUDE_MODELS.iter().find(|m| m.name == name)
}

/// The model's name in the window: its label, or `name` itself when the list lacks it.
pub(crate) fn display_name(name: &str) -> String {
    find(name).map_or_else(|| name.to_string(), |m| m.label.to_string())
}
