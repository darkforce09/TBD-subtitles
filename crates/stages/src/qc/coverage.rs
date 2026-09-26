//! Speech coverage: the stretches where an engine heard words, and the stretches of speech that
//! no cue shows.

use job_model::outputs::{EngineTranscript, TimeSpan};
use subtitle_formats::cue::CueTrack;

use crate::sound_events::candidates::sound_tag;

/// Uncovered stretches shorter than this are not reported.
pub const MIN_UNCOVERED_S: f64 = 1.0;
/// A heard word counts for at most this long: Whisper stretches word ends over the silence after.
pub const MAX_WORD_S: f64 = 1.0;
/// Heard words closer than this form one stretch.
pub const WORD_GAP_S: f64 = 0.5;

/// The stretches where any of `transcripts` heard a word (sound tags such as `*Grunting*` not
/// counted), each word at most `MAX_WORD_S` long, words closer than `WORD_GAP_S` joined, in time
/// order.
pub fn heard_spans(transcripts: &[&EngineTranscript]) -> Vec<TimeSpan> {
    let mut words: Vec<TimeSpan> = transcripts
        .iter()
        .flat_map(|t| t.words())
        .filter(|w| sound_tag(&w.text).is_none())
        .map(|w| TimeSpan::new(w.start_s, w.end_s.clamp(w.start_s, w.start_s + MAX_WORD_S)))
        .collect();
    words.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
    let mut spans: Vec<TimeSpan> = Vec::new();
    for w in words {
        match spans.last_mut() {
            Some(last) if w.start_s - last.end_s < WORD_GAP_S => {
                last.end_s = last.end_s.max(w.end_s)
            }
            _ => spans.push(w),
        }
    }
    spans
}

/// Stretches of `speech` outside `excused` (songs, dropped noise) and outside every cue, each at
/// least `MIN_UNCOVERED_S` long, in time order.
pub fn uncovered(speech: &[TimeSpan], excused: &[TimeSpan], track: &CueTrack) -> Vec<TimeSpan> {
    let rate = track.frame_rate;
    let mut covered: Vec<TimeSpan> = track
        .cues
        .iter()
        .map(|c| TimeSpan::new(rate.seconds(c.start), rate.seconds(c.end)))
        .chain(excused.iter().copied())
        .collect();
    covered.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
    let mut out = Vec::new();
    for region in speech {
        let mut at = region.start_s;
        for c in covered
            .iter()
            .filter(|c| c.end_s > region.start_s && c.start_s < region.end_s)
        {
            if c.start_s > at {
                out.push(TimeSpan::new(at, c.start_s));
            }
            at = at.max(c.end_s);
        }
        if region.end_s > at {
            out.push(TimeSpan::new(at, region.end_s));
        }
    }
    out.retain(|s| s.duration_s() >= MIN_UNCOVERED_S);
    out
}
