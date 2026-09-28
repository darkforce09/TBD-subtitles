//! The log window's actions: open and close it, read the new lines and model calls while it is
//! open, switch views, filter, open a line or a call, clear, and open the log file.
//!
//! **Role:** apply `ShowLog` and each `LogConsoleEvent` to the application's console, and copy
//! the lines and calls logged since the last frame into it.
//!
//! **Position:** called by `TbdSubtitlesApp::apply` and `poll`; reads the process's log buffer
//! through the environment.
//!
//! **Signals and state:** the console's lists, filters, view and selections; Clear empties the
//! log buffer's lines or calls too.
//!
//! **Invariants:** lines and calls are read only while the window is open, and none is lost
//! meanwhile: the buffer keeps them until the window reads them or they age out of it.

use crate::application::TbdSubtitlesApp;
use crate::application::background::Opening;
use crate::log_console::events::LogConsoleEvent;
use crate::log_console::models::console::ConsoleView;

impl TbdSubtitlesApp {
    /// Open the log window, or close it.
    pub(crate) fn show_log(&mut self, open: bool) {
        self.log_window = open;
        if open {
            poll_log(self);
        }
    }

    pub(crate) fn apply_log_console(&mut self, event: LogConsoleEvent) {
        let console = &mut self.console;
        match event {
            LogConsoleEvent::View(view) => console.view = view,
            LogConsoleEvent::Level(level) => console.activity.set_level(level),
            LogConsoleEvent::Who(who) => console.activity.set_who(who),
            LogConsoleEvent::Search(search) => console.set_search(search),
            LogConsoleEvent::SelectLine(seq) => console.activity.select(seq),
            LogConsoleEvent::SelectCall(id) => console.calls.select(id),
            LogConsoleEvent::ShowCall(id) => console.show_call(id),
            LogConsoleEvent::Clear => match console.view {
                ConsoleView::Activity => {
                    self.env.log.clear();
                    console.activity.clear();
                }
                ConsoleView::Calls => {
                    self.env.log.clear_calls();
                    console.calls.clear();
                }
            },
            LogConsoleEvent::OpenLogFile => {
                if let Some(path) = self.env.log_file.clone() {
                    self.open_with_desktop(Opening::Read, &path);
                }
            }
        }
    }
}

/// Copy the lines and calls logged since the last read into the console, while the log window
/// is open.
pub(crate) fn poll_log(app: &mut TbdSubtitlesApp) {
    if app.log_window {
        let lines = app.env.log.since(app.console.activity.next());
        app.console.activity.append(lines);
        let calls = app.env.log.calls_since(app.console.calls.next());
        app.console.calls.append(calls);
    }
}
