//! What the model is told in each of Fix It's passes: the rules and the answer's JSON Schema.
//!
//! **Role:** hold the system prompt of the brief, of each problem family's repair and of the
//! judge, the rules they share, and the three schemas.
//!
//! **Position:** used by `brief.rs`, `repair.rs` and `judge.rs`.
//!
//! **Signals and state:** none; text and JSON.
//!
//! **Invariants:** every repair prompt carries the shared hard rules (only heard words, no
//! rewording, no word moved between lines); an answer's flags never include `UNSURE`.

use job_model::outputs::FixFamily;
use serde_json::{Value, json};

/// The first pass: the context of the video and the lines that do not fit.
pub const BRIEF: &str = "\
You prepare to fix the English subtitles of one video, an English dub. You get the video's file \
name and folder, the series glossary, and every subtitle line in order as `ID m:ss.d [FLAGS] | \
text`. Work out the context from them and from what you know of the series: which show, arc and \
episode this is, who speaks, and what happens, scene by scene. Then read the lines as a whole. A \
line that makes no sense where it stands (a likely mishearing, words that break the conversation) \
is a suspect: give its id and say why in a few words. List only real suspects, at most 30, and \
never propose wording. Ways of speaking that are meant (a stutter, a catchphrase, a repeated word, \
a verbal tic) go in speech_habits with who has them, so nobody fixes them. Keep the summary under \
250 words. Answer only with the JSON.";

/// What the brief adds when the video comes in parts.
pub const BRIEF_PART: &str = "\n\nThe video is long, so its lines come in parts. The summaries \
of the earlier parts are given; brief this part, and name suspects from this part only.";

/// The rules every repair family shares.
const REPAIR: &str = "\
You fix flagged lines of the English subtitles of an English-dubbed video. The brief tells you \
the show, the cast and the scenes.

Each flagged line shows: its problems; what each engine heard (P is the main engine, W the second \
one; p and w are the same stretch heard again on the isolated voice track); the line as the \
subtitles have it now, with its flags; its timing, where a word marked ~ was placed without the \
aligner (the aligner could not find it in the audio); and the lines around it.

Hard rules:
- Build the line only from words an engine heard in this line or the lines next to it, or from \
the glossary. Never add a word, reword, paraphrase, or move words from one line to another.
- Keep the speaker's words and meaning, meaningful interjections and hesitations, and every speech \
habit the brief lists.
- Fix capitals, punctuation and the spelling of glossary names where needed; do not restyle what \
is already right.
- `||` inside a line marks a second speaker. Flags: NARR the narrator, SPK the line starts with a \
different speaker than the line before, LYRIC sung lyrics (left out of the subtitles), DROP noise \
or speech not worth a subtitle (removes the line).
- Return every listed line once as {id, t, f, why}: t the whole line, f its flags, why one short \
sentence. Returning a line exactly as it is now is allowed and often right; then say why it is \
right.
Answer only with the JSON.";

const WORDS: &str = "\n\nThese lines have word problems: the language model was unsure, used a \
word no engine heard, replaced a word both engines agreed on, or the line does not fit the \
conversation. Choose between the heard variants by sense and by the brief; put back a heard word \
that was wrongly replaced; a glossary name may replace how the engines spelled it. If no heard \
variant makes sense, return the line unchanged.";

const TIMING: &str = "\n\nThese lines have timing or layout problems in the finished subtitles. \
The app times every line you return again on its own, from the words you give, so most problems \
need no word change: return the line unchanged and it is timed again. Change words only where \
they cause the problem:
- Speech with no subtitle: a heard word at the start or end of a nearby line was left out (often \
a short 'Uh', 'Oh' or a name); put it back in that line.
- A subtitle too short or squeezed: words only one engine heard that the aligner could not place \
(marked ~) may belong to background voices or another moment; take them out only when the scene \
shows they are not this speaker's line. A short interjection may stand alone.
- A wrong SPK flag or a missing || can keep two speakers' words from sharing a subtitle; fix the \
flags.
- DROP a line only when it is noise, not speech.";

const READING_SPEED: &str = "\n\nThese lines read faster than 20 characters per second. The \
subtitles already stretch each line into the pauses around it; a line stays too fast only when \
the speaker talks fast. You may only take words out, never add or replace one, and never change \
the flags: pure filler (uh, um), a stutter, or a repeated word that adds nothing. Never remove a \
name, a negation, a word that carries meaning, or a speech habit the brief lists. When in doubt, \
return the line unchanged; most lines should stay as they are.";

/// The last pass: each change held against the line before it.
pub const JUDGE: &str = "\
You check changes another pass made to the English subtitles of an English-dubbed video, against \
what the speech engines heard and the brief. For each line you get the problems it had, the line \
before and after the change, why it was changed, the heard words it now leaves out, what each \
engine heard (P main, W second, p and w heard again on the voice track), and the lines around it.

Accept a change only if all of these hold: every word was heard by an engine or is a glossary \
name, and says what the speaker said; the line reads right in the scene and for that character; \
each word taken out is truly filler, a stutter or an empty repeat, a stray from another voice, or \
a mishearing, never meaning, a name, or a speech habit the brief lists; the flags match who \
speaks. Turn down restyling that fixes nothing, any rewording, and anything you doubt.

Return every listed line once as {id, accept, why}, why one short sentence. Answer only with the \
JSON.";

/// The system prompt of one repair family.
pub fn repair(family: FixFamily) -> String {
    let own = match family {
        FixFamily::Words => WORDS,
        FixFamily::Timing => TIMING,
        FixFamily::ReadingSpeed => READING_SPEED,
    };
    format!("{REPAIR}{own}")
}

/// The brief's JSON Schema.
pub fn brief_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "show": {"type": "string"},
            "episode": {"type": "string"},
            "cast": {"type": "array", "items": {"type": "string"}},
            "summary": {"type": "string"},
            "speech_habits": {"type": "array", "items": {"type": "string"}},
            "suspects": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {"id": {"type": "string"}, "why": {"type": "string"}},
                    "required": ["id", "why"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["show", "episode", "cast", "summary", "speech_habits", "suspects"],
        "additionalProperties": false
    })
}

/// A repair answer's JSON Schema.
pub fn repair_schema() -> Value {
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
                        "f": {"type": "array", "items": {"type": "string", "enum": ["NARR", "SPK", "LYRIC", "DROP"]}},
                        "why": {"type": "string"}
                    },
                    "required": ["id", "t", "f", "why"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["lines"],
        "additionalProperties": false
    })
}

/// The judge's JSON Schema.
pub fn judge_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "verdicts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "accept": {"type": "boolean"},
                        "why": {"type": "string"}
                    },
                    "required": ["id", "accept", "why"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["verdicts"],
        "additionalProperties": false
    })
}
