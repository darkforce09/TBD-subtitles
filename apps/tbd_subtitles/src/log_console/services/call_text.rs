//! A model call as copyable text: its header, then each part under a heading.
//!
//! **Role:** write a kept call's summary line and the whole call for Copy All.
//!
//! **Position:** used by the log window's Model Calls view.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** every part the model saw or said is copied whole, in the order it was sent.

use crate::core::log_buffer::KeptCall;
use crate::log_console::services::console_text::{header, time};

/// What a call cost and how long it took: `41.3 s · 18234 in, 2210 out · $0.0874`.
pub(crate) fn summary(kept: &KeptCall) -> String {
    let call = &kept.call;
    let mut text = format!("{:.1} s", call.seconds);
    if call.error.is_none() {
        text.push_str(&format!(
            " · {} in, {} out",
            call.input_tokens, call.output_tokens
        ));
    }
    if let Some(usd) = call.cost_usd {
        text.push_str(&format!(" · ${usd:.4}"));
    }
    text
}

/// Where the call was made: the video and step as a header says them.
pub(crate) fn place(kept: &KeptCall) -> String {
    header(kept.video.as_deref(), kept.step.as_deref())
}

/// The whole call, for Copy All.
pub(crate) fn call_text(kept: &KeptCall) -> String {
    let call = &kept.call;
    let outcome = call.error.as_ref().map_or_else(
        || "answered".to_string(),
        |error| format!("failed: {error}"),
    );
    format!(
        "Model call {} at {}\nModel: {}\nFor: {}\nWhere: {}\nOutcome: {outcome} · {}\n\n\
         ## System prompt\n{}\n\n## Message\n{}\n\n## Schema\n{}\n\n## Answer\n{}\n",
        call.id,
        time(kept.elapsed).trim(),
        call.model,
        call.purpose,
        place(kept),
        summary(kept),
        call.system,
        call.message,
        call.schema,
        call.answer
    )
}

#[cfg(test)]
#[path = "tests/call_text.rs"]
mod tests;
