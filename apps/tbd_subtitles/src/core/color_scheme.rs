//! The desktop's light or dark preference, read from the desktop portal and followed as it
//! changes.
//!
//! **Role:** tell the window whether the desktop (KDE's colour scheme) prefers light or dark, once
//! at start and again after every change.
//!
//! **Position:** started by the application when the window opens; the theme in `core::ui` turns
//! each [`Scheme`] into colours. Uses `ashpd`'s `settings` portal (zbus, pure Rust).
//!
//! **Signals and state:** one thread for the window's lifetime, blocked on the portal's change
//! stream; each value goes through a channel, then the window is woken.
//!
//! **Invariants:** "no preference" is light; a desktop without the portal leaves the window light
//! and closes the channel.

use std::sync::mpsc::{Receiver, Sender, channel};

use ashpd::desktop::settings::{ColorScheme, Settings};
use futures_util::StreamExt as _;

use crate::core::background::Wake;

/// The colour scheme the window follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Scheme {
    #[default]
    Light,
    Dark,
}

impl Scheme {
    /// The scheme for the portal's preference; no preference is light.
    pub(crate) fn from_portal(preference: ColorScheme) -> Scheme {
        match preference {
            ColorScheme::PreferDark => Scheme::Dark,
            ColorScheme::PreferLight | ColorScheme::NoPreference => Scheme::Light,
        }
    }
}

/// Read the desktop's scheme on a thread and then follow its changes: each value arrives on the
/// returned channel, then `wake` runs. The channel closes when the portal cannot be reached.
pub(crate) fn watch(wake: Wake) -> Receiver<Scheme> {
    let (send, schemes) = channel();
    std::thread::spawn(move || {
        if let Err(error) = pollster::block_on(follow(&send, &wake)) {
            tracing::warn!(%error, "the desktop's colour scheme cannot be read; the window stays light");
        }
    });
    schemes
}

/// Subscribe to changes, send the current scheme, then send each change until the window stops
/// listening.
async fn follow(send: &Sender<Scheme>, wake: &Wake) -> Result<(), ashpd::Error> {
    let settings = Settings::new().await?;
    let changes = settings.receive_color_scheme_changed().await?;
    let current = settings.color_scheme().await?;
    if send.send(Scheme::from_portal(current)).is_err() {
        return Ok(());
    }
    wake();
    let mut changes = std::pin::pin!(changes);
    while let Some(preference) = changes.next().await {
        if send.send(Scheme::from_portal(preference)).is_err() {
            break;
        }
        wake();
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/color_scheme.rs"]
mod tests;
