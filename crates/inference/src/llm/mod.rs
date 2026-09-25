//! The language-model backends behind one trait: each takes the diff sheet and returns the
//! adjudicated utterances as JSON.

pub mod claude_cli;
pub mod mistral_rs;
