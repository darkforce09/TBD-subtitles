use super::*;

fn correction(id: &str, chosen: Chosen) -> Correction {
    Correction {
        id: id.into(),
        text: "Does that mean you're gonna stay?".into(),
        flags: vec!["SPK".into()],
        chosen,
    }
}

fn fix(model: &str) -> Chosen {
    Chosen::FixIt {
        model: model.into(),
        why: "restores the heard Uh".into(),
    }
}

#[test]
fn an_earlier_review_json_still_reads() {
    let json = r#"{"lines":[
        {"id":"U0001","text":"Hi.","flags":[],"chosen":{"engine":"P"}},
        {"id":"U0002","text":"Bye.","chosen":"typed"}
    ]}"#;
    let corrections: Corrections = serde_json::from_str(json).unwrap();
    assert_eq!(corrections.lines[0].chosen, Chosen::Engine("P".into()));
    assert_eq!(corrections.lines[1].chosen, Chosen::Typed);
    assert!(corrections.lines[1].flags.is_empty());
    assert_eq!(corrections.owner_count(), 2);
}

#[test]
fn fix_it_choices_round_trip_through_json() {
    let corrections = Corrections {
        lines: vec![
            correction("U0001", fix("opus")),
            correction(
                "U0002",
                Chosen::KeptFixIt {
                    model: "opus".into(),
                    why: "drops a stray".into(),
                },
            ),
        ],
    };
    let json = serde_json::to_string(&corrections).unwrap();
    assert!(json.contains(r#""fix_it":{"model":"opus""#), "{json}");
    assert!(json.contains(r#""kept_fix_it""#), "{json}");
    let back: Corrections = serde_json::from_str(&json).unwrap();
    assert_eq!(back, corrections);
}

#[test]
fn only_a_fix_the_owner_has_not_kept_is_unchecked() {
    let corrections = Corrections {
        lines: vec![
            correction("U0001", fix("opus")),
            correction(
                "U0002",
                Chosen::KeptFixIt {
                    model: "opus".into(),
                    why: String::new(),
                },
            ),
            correction("U0003", Chosen::Typed),
            correction("U0004", Chosen::Engine("adjudicated".into())),
        ],
    };
    assert!(!corrections.by_owner("U0001"));
    assert!(corrections.by_owner("U0002"));
    assert!(corrections.by_owner("U0003"));
    assert!(corrections.by_owner("U0004"));
    assert!(!corrections.by_owner("U0009"));
    assert_eq!(corrections.owner_count(), 3);
    assert_eq!(corrections.unchecked_fix_count(), 1);
}
