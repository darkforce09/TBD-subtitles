use job_model::outputs::{ChunkWords, TimedWord};

use super::*;

fn word(text: &str, start_s: f64, end_s: f64) -> TimedWord {
    TimedWord {
        text: text.into(),
        start_s,
        end_s,
        confidence: None,
    }
}

fn utterance(id: &str, start_s: f64, end_s: f64) -> Utterance {
    Utterance {
        id: id.into(),
        start_s,
        end_s,
        words: vec![word("hey", start_s, end_s)],
        locked: vec![true],
        line: String::new(),
        hypotheses: vec![],
    }
}

fn line(id: &str, flags: &[&str]) -> Line {
    Line {
        id: id.into(),
        t: "hey".into(),
        f: flags.iter().map(|s| s.to_string()).collect(),
    }
}

fn event(label: &str, stem: &str, start_s: f64, end_s: f64, peak: f32) -> SoundEvent {
    SoundEvent {
        label: label.into(),
        stem: stem.into(),
        start_s,
        end_s,
        peak,
    }
}

fn whisper(words: Vec<TimedWord>) -> EngineTranscript {
    EngineTranscript {
        engine: "whisper".into(),
        input: "mix".into(),
        chunks: vec![ChunkWords {
            span: TimeSpan::new(0.0, 100.0),
            words,
        }],
    }
}

#[test]
fn lyric_runs_become_one_song_each() {
    let sheet = [
        utterance("U1", 0.0, 4.0),
        utterance("U2", 6.0, 9.0),
        utterance("U3", 20.0, 22.0),
        utterance("U4", 30.0, 31.0),
    ];
    let lines = [
        line("U1", &["LYRIC"]),
        line("U2", &["LYRIC"]),
        line("U3", &[]),
        line("U4", &["LYRIC"]),
    ];
    assert_eq!(
        song_spans(&sheet, &lines),
        vec![TimeSpan::new(0.0, 9.0), TimeSpan::new(30.0, 31.0)]
    );
}

#[test]
fn music_and_events_inside_songs_are_not_candidates() {
    let sheet = [utterance("U1", 0.0, 10.0)];
    let lines = [line("U1", &["LYRIC"])];
    let events = [
        event("Music", "background", 0.0, 60.0, 0.9),
        event("Explosion", "background", 2.0, 3.0, 0.9),
        event("Explosion", "background", 40.0, 41.0, 0.8),
    ];
    let (found, songs) = candidates(&events, &sheet, &lines, &whisper(vec![]));
    assert_eq!(songs.len(), 1);
    let labels: Vec<(&str, CandidateKind)> =
        found.iter().map(|c| (c.label.as_str(), c.kind)).collect();
    assert_eq!(
        labels,
        vec![
            ("Song", CandidateKind::Song),
            ("Explosion", CandidateKind::Effect)
        ]
    );
    assert_eq!(found[0].id, "S001");
    assert_eq!(found[1].id, "S002");
}

#[test]
fn voices_during_speech_and_weak_groans_are_dropped() {
    let sheet = [utterance("U1", 10.0, 12.0)];
    let lines = [line("U1", &[])];
    let events = [
        event("Screaming", "vocals", 10.5, 11.5, 0.9),
        event("Screaming", "vocals", 20.0, 21.0, 0.9),
        event("Groan", "vocals", 30.0, 30.6, 0.9),
        event("Groan", "vocals", 40.0, 41.5, 0.5),
        event("Groan", "vocals", 50.0, 51.5, 0.7),
    ];
    let (found, _) = candidates(&events, &sheet, &lines, &whisper(vec![]));
    let kept: Vec<(String, f64)> = found.iter().map(|c| (c.label.clone(), c.start_s)).collect();
    assert_eq!(
        kept,
        vec![("Screaming".to_string(), 20.0), ("Groan".to_string(), 50.0)]
    );
}

#[test]
fn same_class_events_merge_across_short_gaps() {
    let events = [
        event("Door", "background", 5.0, 5.5, 0.6),
        event("Door", "background", 6.0, 6.4, 0.8),
        event("Door", "background", 9.0, 9.4, 0.7),
    ];
    let (found, _) = candidates(&events, &[], &[], &whisper(vec![]));
    assert_eq!(found.len(), 2);
    assert_eq!(
        (found[0].start_s, found[0].end_s, found[0].peak),
        (5.0, 6.4, 0.8)
    );
}

#[test]
fn whisper_sound_tags_become_candidates() {
    assert_eq!(sound_tag("*Grunting*"), Some("grunting".into()));
    assert_eq!(sound_tag("[laughs]."), Some("laughs".into()));
    assert_eq!(sound_tag("(sighs)"), Some("sighs".into()));
    assert_eq!(sound_tag("hello"), None);
    assert_eq!(sound_tag("**"), None);
    let (found, _) = candidates(
        &[],
        &[],
        &[],
        &whisper(vec![word("*Coughing*", 3.0, 3.5), word("okay", 4.0, 4.2)]),
    );
    assert_eq!(found.len(), 1);
    assert_eq!(
        (found[0].kind, found[0].label.as_str()),
        (CandidateKind::Tag, "coughing")
    );
}

#[test]
fn an_instrumental_break_does_not_split_a_song_but_a_long_one_does() {
    let sheet = [
        utterance("U1", 0.0, 17.0),
        utterance("U2", 20.0, 21.0),
        utterance("U3", 24.0, 86.0),
        utterance("U4", 130.0, 140.0),
    ];
    let lines = [
        line("U1", &["LYRIC"]),
        line("U2", &["DROP"]),
        line("U3", &["LYRIC"]),
        line("U4", &["LYRIC"]),
    ];
    assert_eq!(
        song_spans(&sheet, &lines),
        vec![TimeSpan::new(0.0, 86.0), TimeSpan::new(130.0, 140.0)]
    );
}
