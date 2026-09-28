//! A clip being played: what it covers and the latest picture frame.

/// One decoded preview frame, RGBA, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Frame {
    /// A number no other frame of the window has, so a view uploads each frame once.
    pub(crate) serial: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
}

/// Which sound a clip plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sound {
    /// The video's own track, as a viewer hears it.
    Mix,
    /// The separated vocal stem, the voices alone.
    Voices,
}
