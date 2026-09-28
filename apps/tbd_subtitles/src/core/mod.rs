//! What every module of the app shares: logging, work on background threads, the desktop portal,
//! the desktop's colour scheme, how numbers are written, and the shared look of the window.

pub(crate) mod background;
pub(crate) mod color_scheme;
pub(crate) mod format;
pub(crate) mod logging;
pub(crate) mod portal;
pub(crate) mod ui;
