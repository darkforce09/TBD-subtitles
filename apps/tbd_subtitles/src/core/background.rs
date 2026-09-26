//! Work the window hands to threads of its own: how a thread tells the window it has news.

use std::sync::Arc;

/// Asks the window to draw a frame soon, so it reads what a thread sent; called from any thread.
pub(crate) type Wake = Arc<dyn Fn() + Send + Sync>;

/// A wake that does nothing, for tests.
#[cfg(test)]
pub(crate) fn no_wake() -> Wake {
    Arc::new(|| {})
}
