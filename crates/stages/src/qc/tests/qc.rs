use job_model::outputs::{
    AlignedUtterance, AlignedWord, CandidateKind, Findings, Line, SoundCandidate,
};
use subtitle_formats::cue::{Cue, CueLine, FrameRate};

use super::*;

fn cue(start: u64, end: u64, lines: &[&str]) -> Cue {
    Cue {
        start,
        end,
        lines: lines.iter().map(|l| CueLine::plain(*l)).collect(),
        kind: CueKind::Dialogue,
    }
}

fn track(cues: Vec<Cue>) -> CueTrack {
    CueTrack {
        frame_rate: FrameRate::FILM,
        cues,
    }
}

fn checks_of(findings: &[QcFinding]) -> Vec<QcCheck> {
    findings.iter().map(|f| f.check).collect()
}

#[test]
fn a_clean_track_has_no_cue_findings() {
    let rules = FrameRules::new(FrameRate::FILM, 60.0);
    let clean = track(vec![
        cue(24, 72, &["Hello there."]),
        cue(74, 120, &["-Ready?", "-Yes!"]),
    ]);
    assert!(cue_findings(&clean, &rules).is_empty());
}

#[test]
fn each_layout_rule_is_flagged() {
    let rules = FrameRules::new(FrameRate::FILM, 10.0);
    let long = "x".repeat(43);
    let bad = track(vec![
        cue(0, 10, &["short"]),
        cue(11, 40, &["gap of one", "a", "b"]),
        cue(35, 70, &[long.as_str()]),
        cue(72, 250, &["too long and past the end"]),
        cue(
            252,
            272,
            &["This line is far too long to read in so short a time."],
        ),
        cue(274, 300, &[" "]),
    ]);
    let found = checks_of(&cue_findings(&bad, &rules));
    for check in [
        QcCheck::TooShort,
        QcCheck::GapTooSmall,
        QcCheck::TooManyLines,
        QcCheck::Overlap,
        QcCheck::LineTooLong,
        QcCheck::TooLong,
        QcCheck::PastEnd,
        QcCheck::TooFast,
        QcCheck::Empty,
    ] {
        assert!(found.contains(&check), "{check:?} not in {found:?}");
    }
}

#[test]
fn speech_without_a_cue_is_found_outside_songs_and_dropped_noise() {
    let t = track(vec![cue(24, 48, &["covered"])]);
    let speech = [
        TimeSpan::new(1.0, 2.0),
        TimeSpan::new(5.0, 8.0),
        TimeSpan::new(10.0, 12.0),
        TimeSpan::new(20.0, 20.5),
    ];
    let excused = [TimeSpan::new(10.0, 12.0)];
    assert_eq!(
        coverage::uncovered(&speech, &excused, &t),
        vec![TimeSpan::new(5.0, 8.0)]
    );
}

fn word(text: &str, start_s: f64, source: TimingSource) -> AlignedWord {
    AlignedWord {
        text: text.into(),
        start_s,
        end_s: start_s + 0.2,
        source,
    }
}

#[test]
fn the_whole_check_reports_lines_offsets_and_counts() {
    let mut boom = cue(100, 130, &["[explosion]"]);
    boom.kind = CueKind::Sound;
    let t = track(vec![cue(24, 72, &["Hello there."]), boom]);
    let aligned = Aligned {
        utterances: vec![
            AlignedUtterance {
                id: "U1".into(),
                words: vec![
                    word("Hello", 1.0, TimingSource::Ctc),
                    word("there.", 1.3, TimingSource::Ctc),
                ],
                speaker_starts: vec![],
                new_speaker: false,
                narrator: false,
                unsure: true,
            },
            AlignedUtterance {
                id: "U2".into(),
                words: vec![word("Hm.", 9.0, TimingSource::Backbone)],
                speaker_starts: vec![],
                new_speaker: false,
                narrator: false,
                unsure: false,
            },
        ],
        offset_s: Some(-0.045),
        ..Aligned::default()
    };
    let adjudicated = AdjudicationPass {
        lines: vec![Line {
            id: "U1".into(),
            t: "Hello there.".into(),
            f: vec!["UNSURE".into()],
        }],
        findings: Findings {
            novel: vec![("U1".into(), "there".into())],
            ..Findings::default()
        },
        failed_calls: vec!["U0061..: timeout".into()],
        ..AdjudicationPass::default()
    };
    let sound_cues = SoundCues {
        candidates: vec![SoundCandidate {
            id: "S001".into(),
            kind: CandidateKind::Song,
            label: "Song".into(),
            start_s: 30.0,
            end_s: 40.0,
            peak: 1.0,
        }],
        ..SoundCues::default()
    };
    let speech = SpeechPlan {
        regions: vec![TimeSpan::new(30.0, 40.0), TimeSpan::new(50.0, 52.0)],
        ..SpeechPlan::default()
    };
    let starts: HashMap<String, f64> = [("U1".to_string(), 1.0), ("U2".to_string(), 9.0)].into();
    let report = check(&QcInput {
        track: &t,
        aligned: &aligned,
        adjudicated: &adjudicated,
        sound_cues: &sound_cues,
        speech: &speech,
        heard: &[TimeSpan::new(30.0, 40.0), TimeSpan::new(50.0, 51.5)],
        dropped: &[],
        utterance_starts: &starts,
        duration_s: 60.0,
    });
    let found = checks_of(&report.findings);
    for check in [
        QcCheck::Unsure,
        QcCheck::Novel,
        QcCheck::WeakTiming,
        QcCheck::Offset,
        QcCheck::FailedCall,
        QcCheck::UncoveredSpeech,
    ] {
        assert!(found.contains(&check), "{check:?} not in {found:?}");
    }
    assert!(
        report
            .findings
            .windows(2)
            .all(|w| w[0].time_s <= w[1].time_s)
    );
    let s = &report.summary;
    assert_eq!((s.cues, s.dialogue_cues, s.unsure, s.novel), (2, 1, 1, 1));
    assert_eq!(s.words_by_source["ctc"], 2);
    assert_eq!(s.words_by_source["interpolated"], 0);
    assert!((s.uncovered_speech_s - 1.5).abs() < 1e-9);
    assert!((s.voice_without_cue_s - 2.0).abs() < 1e-9);
    assert_eq!(s.cps_ok_share, 1.0);
}

#[test]
fn heard_spans_join_close_words_of_every_engine_and_skip_sound_tags() {
    use job_model::outputs::{ChunkWords, EngineTranscript, TimedWord};
    let transcript = |words: &[(&str, f64, f64)]| EngineTranscript {
        engine: "e".into(),
        input: "mix".into(),
        chunks: vec![ChunkWords {
            span: TimeSpan::new(0.0, 100.0),
            words: words
                .iter()
                .map(|&(t, s, e)| TimedWord {
                    text: t.into(),
                    start_s: s,
                    end_s: e,
                    confidence: None,
                })
                .collect(),
        }],
    };
    let p = transcript(&[("hey", 1.0, 1.2), ("you", 1.4, 1.6), ("later", 5.0, 5.3)]);
    let w = transcript(&[
        ("*Grunting*", 3.0, 3.5),
        ("there", 1.7, 2.0),
        ("long", 8.0, 12.0),
    ]);
    assert_eq!(
        coverage::heard_spans(&[&p, &w]),
        vec![
            TimeSpan::new(1.0, 2.0),
            TimeSpan::new(5.0, 5.3),
            TimeSpan::new(8.0, 9.0)
        ]
    );
}
