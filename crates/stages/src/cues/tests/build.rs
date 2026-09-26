use job_model::outputs::{AlignedUtterance, AlignedWord, CandidateKind, ShotCut, TimingSource};

use super::*;

fn utterance(id: &str, text: &str, start_s: f64, step: f64) -> AlignedUtterance {
    AlignedUtterance {
        id: id.into(),
        words: text
            .split_whitespace()
            .enumerate()
            .map(|(i, t)| AlignedWord {
                text: t.into(),
                start_s: start_s + i as f64 * step,
                end_s: start_s + (i as f64 + 0.8) * step,
                source: TimingSource::Ctc,
            })
            .collect(),
        speaker_starts: vec![],
        new_speaker: false,
        narrator: false,
        unsure: false,
    }
}

#[test]
fn a_whole_scene_builds_ordered_frame_true_cues() {
    let aligned = Aligned {
        utterances: vec![
            utterance("U1", "Law, the Birdcage is closing in!", 10.0, 0.3),
            utterance(
                "U2",
                "Then we have to reach Doflamingo before it closes on the whole island.",
                12.4,
                0.25,
            ),
            utterance("U3", "Go!", 20.0, 0.3),
        ],
        ..Aligned::default()
    };
    let sounds = [SoundCue {
        candidate: "S001".into(),
        kind: CandidateKind::Effect,
        start_s: 25.0,
        end_s: 26.0,
        text: "[explosion]".into(),
    }];
    let shots = ShotChanges {
        cuts: vec![ShotCut {
            time_s: 9.8,
            score: 30.0,
        }],
    };
    let built = build(&aligned, &sounds, &shots, 20.0, FrameRate::FILM, 60.0);
    let cues = &built.track.cues;
    assert!(built.dropped_sounds.is_empty());
    assert_eq!(
        cues[0].start, 235,
        "starts on the cut 0.2 s before the speech"
    );
    for pair in cues.windows(2) {
        assert!(pair[0].end + 2 <= pair[1].start, "{pair:?}");
    }
    for cue in cues {
        assert!(
            cue.lines.len() <= 2 && cue.lines.iter().all(|l| l.chars() <= 42),
            "{cue:?}"
        );
        assert!(cue.frames() >= 20 && cue.frames() <= 168, "{cue:?}");
    }
    assert_eq!(cues.last().map(|c| c.kind), Some(CueKind::Sound));
}
