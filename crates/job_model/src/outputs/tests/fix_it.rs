use super::*;

fn line(verdict: FixVerdict) -> LineFix {
    LineFix {
        id: "U0295".into(),
        problems: vec!["heard speech over 1 s with no cue".into()],
        before_text: "Does that mean you're gonna stay?".into(),
        before_flags: vec!["SPK".into()],
        after_text: "Uh, does that mean you're gonna stay?".into(),
        after_flags: vec!["SPK".into()],
        steps: vec![FixStep {
            family: FixFamily::Timing,
            text: "Uh, does that mean you're gonna stay?".into(),
            flags: vec!["SPK".into()],
            why: "P heard Uh where the subtitle starts late.".into(),
        }],
        refused: Vec::new(),
        removed: Vec::new(),
        verdict,
        applied: false,
    }
}

#[test]
fn only_kept_and_accepted_lines_get_a_correction() {
    let why = || "reason".to_string();
    assert!(FixVerdict::Kept { why: why() }.writes_correction());
    assert!(FixVerdict::Accepted { why: why() }.writes_correction());
    assert!(!FixVerdict::TurnedDown { why: why() }.writes_correction());
    assert!(!FixVerdict::NotJudged { why: why() }.writes_correction());
    assert!(!FixVerdict::Unchanged.writes_correction());
}

#[test]
fn the_owner_reads_the_steps_reasons_or_the_kept_reason() {
    let accepted = line(FixVerdict::Accepted {
        why: "fits the scene".into(),
    });
    assert!(accepted.changed());
    assert_eq!(accepted.why(), "P heard Uh where the subtitle starts late.");
    let kept = line(FixVerdict::Kept {
        why: "Re-timed alone.".into(),
    });
    assert_eq!(kept.why(), "Re-timed alone.");
}

#[test]
fn a_record_round_trips_through_json() {
    let record = FixRecord {
        model: "opus".into(),
        video: "[Muhn Pace] Dressrosa 12".into(),
        brief: FixBrief {
            show: "One Piece".into(),
            suspects: vec![Suspect {
                id: "U0061".into(),
                why: "crowd line".into(),
            }],
            ..FixBrief::default()
        },
        lines: vec![line(FixVerdict::Accepted { why: "ok".into() })],
        calls: 4,
        ..FixRecord::default()
    };
    let json = serde_json::to_string(&record).unwrap();
    assert!(json.contains(r#""family":"timing""#), "{json}");
    assert!(json.contains(r#""accepted":{"why":"ok"}"#), "{json}");
    let back: FixRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(back, record);
}
