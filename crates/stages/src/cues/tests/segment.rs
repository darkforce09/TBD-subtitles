use job_model::outputs::{AlignedUtterance, TimingSource};
use subtitle_formats::cue::FrameRate;

use super::*;

/// Words spread evenly from `start_s`, `step` seconds apart.
fn words(text: &str, start_s: f64, step: f64) -> Vec<AlignedWord> {
    text.split_whitespace()
        .enumerate()
        .map(|(i, t)| AlignedWord {
            text: t.into(),
            start_s: start_s + i as f64 * step,
            end_s: start_s + i as f64 * step + step * 0.8,
            source: TimingSource::Ctc,
        })
        .collect()
}

fn aligned(utterances: Vec<AlignedUtterance>) -> Aligned {
    Aligned {
        utterances,
        ..Aligned::default()
    }
}

fn utterance(
    text: &str,
    start_s: f64,
    step: f64,
    speaker_starts: Vec<usize>,
    narrator: bool,
) -> AlignedUtterance {
    AlignedUtterance {
        id: "U1".into(),
        words: words(text, start_s, step),
        speaker_starts,
        new_speaker: false,
        narrator,
        unsure: false,
    }
}

fn rules() -> FrameRules {
    FrameRules::new(FrameRate::FILM, 3600.0)
}

#[test]
fn a_short_utterance_is_one_unit() {
    let units = units(&aligned(vec![utterance(
        "Hey, wait for me!",
        1.0,
        0.3,
        vec![],
        false,
    )]));
    assert_eq!(units.len(), 1);
    assert_eq!(units[0].words.len(), 4);
    assert!(!units[0].speaker_change);
}

#[test]
fn a_long_turn_splits_at_the_last_sentence_end_that_fits() {
    let text = "We have to hurry. The Birdcage is closing in on the whole island and nobody can stop it now. Run!";
    let units = units(&aligned(vec![utterance(text, 0.0, 0.25, vec![], false)]));
    let texts: Vec<String> = units.iter().map(|u| u.text()).collect();
    assert_eq!(texts[0], "We have to hurry.");
    assert!(
        texts
            .iter()
            .all(|t| line_break::fits(&t.split(' ').collect::<Vec<_>>()))
    );
    assert_eq!(texts.concat().replace(' ', ""), text.replace(' ', ""));
}

#[test]
fn a_unit_never_holds_more_than_its_speech_limit() {
    // Slow speech: 20 short words over 10 s fit two lines but not 6.4 s.
    let text = "no no no no no no no no no no no no no no no no no no no no";
    let units = units(&aligned(vec![utterance(text, 0.0, 0.5, vec![], false)]));
    assert!(units.len() >= 2);
    assert!(
        units
            .iter()
            .all(|u| u.end_s() - u.start_s() <= MAX_SPEECH_S)
    );
}

#[test]
fn speaker_changes_start_new_units() {
    let units = units(&aligned(vec![utterance(
        "Are you coming? In a minute.",
        0.0,
        0.2,
        vec![3],
        false,
    )]));
    assert_eq!(units.len(), 2);
    assert!(units[1].speaker_change);
}

#[test]
fn quick_exchanges_share_a_cue_with_dashes() {
    let units = units(&aligned(vec![utterance(
        "Ready? Yes!",
        10.0,
        0.3,
        vec![1],
        false,
    )]));
    let drafts = drafts(&units, &rules());
    assert_eq!(drafts.len(), 1);
    assert_eq!(
        drafts[0].lines,
        vec![CueLine::plain("-Ready?"), CueLine::plain("-Yes!")]
    );
}

#[test]
fn slow_exchanges_and_narration_are_not_paired() {
    // Two seconds apart: each fits a cue of its own.
    let slow = AlignedUtterance {
        id: "U1".into(),
        words: [words("Ready?", 10.0, 0.3), words("Yes!", 12.0, 0.3)].concat(),
        speaker_starts: vec![1],
        new_speaker: false,
        narrator: false,
        unsure: false,
    };
    assert_eq!(drafts(&units(&aligned(vec![slow])), &rules()).len(), 2);
    let narrated = drafts(
        &units(&aligned(vec![utterance(
            "Meanwhile. Yes!",
            10.0,
            0.3,
            vec![1],
            true,
        )])),
        &rules(),
    );
    assert_eq!(narrated.len(), 2);
    assert!(narrated[0].lines.iter().all(|l| l.italic));
}

fn said(id: &str, text: &str, start_s: f64, step: f64, new_speaker: bool) -> AlignedUtterance {
    AlignedUtterance {
        id: id.into(),
        words: words(text, start_s, step),
        speaker_starts: vec![],
        new_speaker,
        narrator: false,
        unsure: false,
    }
}

#[test]
fn a_cramped_answer_shares_a_cue_with_its_question_when_the_speaker_changes() {
    let exchange = aligned(vec![
        said("U1", "What is his deal?", 245.2, 0.25, false),
        said("U2", "I don't know.", 246.5, 0.2, true),
        said("U3", "Said something about Straw Hat.", 247.3, 0.3, true),
    ]);
    let drafts = drafts(&units(&exchange), &rules());
    assert_eq!(
        drafts[0].lines,
        vec![
            CueLine::plain("-What is his deal?"),
            CueLine::plain("-I don't know.")
        ]
    );
    assert_eq!(drafts.len(), 2);
}

#[test]
fn a_cramped_line_of_the_same_speaker_joins_the_next_one() {
    let lines = aligned(vec![
        said("U1", "Hold on!", 10.0, 0.25, false),
        said("U2", "Wait for me!", 10.8, 0.2, false),
        said(
            "U3",
            "Hurry up, everyone, the gates are closing!",
            11.9,
            0.2,
            true,
        ),
    ]);
    let drafts = drafts(&units(&lines), &rules());
    assert_eq!(
        drafts[0].lines,
        vec![CueLine::plain("Hold on! Wait for me!")]
    );
    assert_eq!(drafts.len(), 2);
}

#[test]
fn a_line_with_room_keeps_its_own_cue() {
    let lines = aligned(vec![
        said("U1", "Hold on!", 10.0, 0.25, false),
        said("U2", "Wait for me!", 13.0, 0.2, true),
    ]);
    assert_eq!(drafts(&units(&lines), &rules()).len(), 2);
}
