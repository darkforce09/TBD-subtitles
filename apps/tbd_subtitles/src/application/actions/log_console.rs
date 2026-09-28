//! The log window's actions: open and close it, read the new lines while it is open, filter
//! them, clear them, and open the log file.
//!
//! **Role:** apply `ShowLog` and each `LogConsoleEvent` to the application's console, and copy
//! the lines logged since the last frame into it.
//!
//! **Position:** called by `TbdSubtitlesApp::apply` and `poll`; reads the process's log buffer
//! through the environment.
//!
//! **Signals and state:** the console's lines and filter; Clear empties the log buffer too.
//!
//! **Invariants:** lines are read only while the window is open, and none is lost meanwhile: the
//! buffer keeps them until the window reads them or they age out of it.

use crate::application::TbdSubtitlesApp;
use crate::application::background::Opening;
use crate::log_console::events::LogConsoleEvent;

impl TbdSubtitlesApp {
    /// Open the log window, or close it.
    pub(crate) fn show_log(&mut self, open: bool) {
        self.log_window = open;
        if open {
            poll_log(self);
        }
    }

    pub(crate) fn apply_log_console(&mut self, event: LogConsoleEvent) {
        match event {
            LogConsoleEvent::Level(level) => self.console.set_level(level),
            LogConsoleEvent::Search(search) => self.console.set_search(search),
            LogConsoleEvent::Clear => {
                self.env.log.clear();
                self.console.clear();
            }
            LogConsoleEvent::OpenLogFile => {
                if let Some(path) = self.env.log_file.clone() {
                    self.open_with_desktop(Opening::Read, &path);
                }
            }
        }
    }
}

/// Copy the lines logged since the last read into the console, while the log window is open.
pub(crate) fn poll_log(app: &mut TbdSubtitlesApp) {
    if app.log_window {
        let fresh = app.env.log.since(app.console.next());
        app.console.append(fresh);
    }
}
