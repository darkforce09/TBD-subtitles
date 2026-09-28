use job_model::outputs::{FixVerdict, LineFix};

use super::*;

fn line(id: &str, verdict: FixVerdict) -> LineFix {
    LineFix {
        id: id.into(),
        problems: vec!["too short".into()],
        before_text: "Go!".into(),
        before_flags: Vec::new(),
        after_text: "Uh, go!".into(),
        after_flags: vec!["SPK".into()],
        steps: Vec::new(),
        refused: Vec::new(),
        removed: Vec::new(),
        verdict,
        applied: false,
    }
}

fn correction(id: &str, chosen: Chosen) -> Correction {
    Correction {
        id: id.into(),
        text: "Go.".into(),
        flags: Vec::new(),
        chosen,
    }
}

#[test]
fn the_owner_s_corrections_win_and_an_earlier_fix_is_replaced() {
    let mut corrections = Corrections {
        lines: vec![
            correction("U1", Chosen::Typed),
            correction(
                "U2",
                Chosen::FixIt {
                    model: "sonnet".into(),
                    why: "old".into(),
                },
            ),
        ],
    };
    let accepted = || FixVerdict::Accepted { why: "ok".into() };
    let mut lines = vec![
        line("U1", accepted()),
        line("U2", accepted()),
        line(
            "U3",
            FixVerdict::Kept {
                why: "Timed again alone.".into(),
            },
        ),
        line("U4", FixVerdict::TurnedDown { why: "no".into() }),
    ];
    let merged = merge(&mut corrections, &mut lines, "opus");
    assert_eq!(merged.applied, ["U2", "U3"]);
    assert_eq!(merged.kept_yours, ["U1"]);
    assert_eq!(corrections.get("U1").unwrap().chosen, Chosen::Typed);
    let u2 = corrections.get("U2").unwrap();
    assert_eq!(u2.text, "Uh, go!");
    assert_eq!(u2.flags, ["SPK"]);
    assert!(matches!(&u2.chosen, Chosen::FixIt { model, .. } if model == "opus"));
    assert!(matches!(
        &corrections.get("U3").unwrap().chosen,
        Chosen::FixIt { why, .. } if why == "Timed again alone."
    ));
    assert!(corrections.get("U4").is_none());
    let applied: Vec<bool> = lines.iter().map(|l| l.applied).collect();
    assert_eq!(applied, [false, true, true, false]);
}
