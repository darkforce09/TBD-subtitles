//! What the language model is told: the rules, the answer's JSON Schema, and one batch of the
//! diff sheet with the series glossary.

use serde_json::{Value, json};

use crate::diff_sheet::sheet::Utterance;

/// The rules every backend gets as its system prompt.
pub const SYSTEM: &str = "\
You settle the transcript of an English-dubbed anime episode for subtitles. Each input line is \
one utterance: `ID m:ss.d duration | words`. Words every speech engine agreed on are written \
plainly. Where engines disagree the choices are written inline: `{P:Birdcage|W:bird cage}` means \
engine P heard `Birdcage` and engine W heard `bird cage`; `W:∅` means W heard nothing there; \
`{W:+in}` means W alone heard the extra word `in` at that point. P is the main engine.

For every utterance return exactly one object {\"id\", \"t\", \"f\"}, in input order:
- `t` is the final text. Build it only from words some engine heard in this utterance or the \
lines next to it, or from the glossary. Never add, reword or paraphrase. Choose between heard \
variants by sense and context; fix capitalisation, punctuation and the spelling of glossary names.
- Keep meaningful interjections and hesitations (whoa, hmm, huh, I... I said no!). Drop pure \
filler (uh, um).
- If a second speaker starts inside the utterance, mark the change with `||`.
- `f` lists flags: NARR for the narrator, LYRIC for sung lyrics (they are dropped), DROP for noise \
or gibberish that is not speech, UNSURE when you cannot decide what was said. Otherwise [].
Do not explain. Answer only with the JSON.";

/// The answer's JSON Schema.
pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "lines": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "t": {"type": "string"},
                        "f": {"type": "array", "items": {"type": "string", "enum": ["NARR", "LYRIC", "DROP", "UNSURE"]}}
                    },
                    "required": ["id", "t", "f"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["lines"],
        "additionalProperties": false
    })
}

/// The user message for one batch.
pub fn user_message(glossary: &[&str], batch: &[Utterance]) -> String {
    let mut text = String::from("Glossary (names and terms, spelled right): ");
    text.push_str(&glossary.join(", "));
    text.push_str("\n\nUtterances:\n");
    for u in batch {
        text.push_str(&u.line);
        text.push('\n');
    }
    text
}
