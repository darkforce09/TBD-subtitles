//! What every module of the app shares: logging, work on background threads, the desktop portal,
//! the desktop's colour scheme, how numbers and steps are written, toasts, one window per
//! session, the Dolphin service menu, and the shared look and widgets of the window.

pub(crate) mod background;
pub(crate) mod color_scheme;
pub(crate) mod format;
pub(crate) mod log_buffer;
pub(crate) mod logging;
pub(crate) mod portal;
pub(crate) mod service_menu;
pub(crate) mod single_instance;
pub(crate) mod steps;
pub(crate) mod toast;
pub(crate) mod ui;
