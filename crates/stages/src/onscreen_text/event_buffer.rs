//! Bounded ASS events with adjacent frame coalescing.
//!
//! **Role:** cap typesetting allocations independently of the detector's geometry budget.
//! **Position:** internal storage for the visual typesetter.
//! **Signals and state:** owned events and their allocated text byte count.
//! **Invariants:** no buffer exceeds 128 MiB of text; failed appends leave it unchanged.

use super::TextResult;

pub(super) struct Event {
    pub layer: u8,
    pub start: u64,
    pub end: u64,
    pub text: String,
}

pub(super) struct EventBuffer {
    pub events: Vec<Event>,
    bytes: usize,
    limit: usize,
}

impl Default for EventBuffer {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            bytes: 0,
            limit: 128 * 1024 * 1024,
        }
    }
}

impl std::ops::Deref for EventBuffer {
    type Target = [Event];
    fn deref(&self) -> &Self::Target {
        &self.events
    }
}

impl EventBuffer {
    pub fn push(&mut self, next: Event) -> TextResult<()> {
        if let Some(prior) = self.events.iter_mut().rev().take(2).find(|prior| {
            prior.layer == next.layer && prior.end == next.start && prior.text == next.text
        }) {
            prior.end = next.end;
        } else {
            let size = next.text.capacity() + std::mem::size_of::<Event>();
            self.check(size)?;
            self.bytes += size;
            self.events.push(next);
        }
        Ok(())
    }

    pub fn append(&mut self, other: Self) -> TextResult<()> {
        self.check(other.bytes)?;
        self.bytes += other.bytes;
        self.events.extend(other.events);
        Ok(())
    }

    fn check(&self, additional: usize) -> TextResult<()> {
        if additional > self.limit.saturating_sub(self.bytes) {
            Err("Visual lettering exceeds its bounded event memory. Use nearby text or shorter inputs.".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "tests/event_buffer.rs"]
mod tests;
