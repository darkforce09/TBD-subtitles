//! Where things live in this repository, spelled once for every gate.

/// The top-level folders that hold code. Every tracked folder in them carries a README.md, and
/// README.md is the only Markdown they hold.
pub(crate) const CODE_TREES: &[&str] = &["apps", "crates", "tools"];

/// The documentation root: every document lives under it, and every folder in it has a README.md.
pub(crate) const DOCUMENTATION_ROOT: &str = "documentation";

/// Folders whose documents, other than their README.md index, are frozen records: never
/// reworded, outside the size limit, judged only for their links.
pub(crate) const FROZEN_FOLDERS: &[&str] = &["documentation/research", "documentation/archive"];

/// The project instructions at the repository root, which every agent reads first.
pub(crate) const PROJECT_INSTRUCTIONS: &str = "CLAUDE.md";

/// The product crates, each with its layer: a crate may depend only on crates of a lower layer.
pub(crate) const PRODUCT_LAYERS: &[(&str, u8)] = &[
    ("job_model", 0),
    ("child_process", 0),
    ("media_io", 1),
    ("subtitle_formats", 1),
    ("inference", 1),
    ("stages", 2),
    ("pipeline", 3),
    ("tbd_subtitles", 4),
];

/// The repository tools and the product crates each may depend on.
pub(crate) const TOOL_DEPENDENCIES: &[(&str, &[&str])] = &[
    ("verification_core", &["child_process"]),
    ("repo_gates", &["verification_core"]),
    (
        "stack_spike",
        &[
            "child_process",
            "job_model",
            "media_io",
            "inference",
            "stages",
        ],
    ),
    (
        "stack_spike_ggml",
        &["job_model", "media_io", "inference", "stages"],
    ),
    ("stack_spike_llm", &["job_model", "inference", "stages"]),
];
