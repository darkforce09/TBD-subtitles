//! What the settings view shows about this machine: the models and runtime libraries a job needs,
//! and the checks of the GPU and the programs a job runs.

/// What a download item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemKind {
    /// A model folder in the models folder.
    Model,
    /// A runtime archive unpacked in the runtime folder.
    Runtime,
}

/// One model folder or runtime archive a job needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadItem {
    pub(crate) kind: ItemKind,
    pub(crate) id: String,
    pub(crate) bytes: u64,
    pub(crate) present: bool,
}

/// How a check came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckState {
    Ok,
    /// Jobs can run, with a caveat.
    Warning,
    /// Jobs cannot run until this is fixed.
    Failed,
}

/// One check: what was looked at, how it came out, and what was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Check {
    pub(crate) name: &'static str,
    pub(crate) state: CheckState,
    pub(crate) detail: String,
}
