use job_model::outputs::{Chosen, Correction, Findings};

use super::*;

fn utterance(id: &str, heard: &[&str]) -> Utterance {
    Utterance {
        id: id.into(),
        start_s: 0.0,
        end_s: 1.0,
        words: Vec::new(),
        locked: Vec::new(),
        line: String::new(),
        hypotheses: vec![("P".into(), heard.iter().map(|s| s.to_string()).collect())],
    }
}

fn line(id: &str, t: &str) -> Line {
    Line {
        id: id.into(),
        t: t.into(),
        f: Vec::new(),
    }
}

fn correction(id: &str, text: &str, chosen: Chosen) -> Correction {
    Correction {
        id: id.into(),
        text: text.into(),
        flags: Vec::new(),
        chosen,
    }
}

#[test]
fn owner_lines_are_settled_and_fix_it_lines_are_checked_again() {
    let sheet = vec![
        utterance("U1", &["Go", "Luffy"]),
        utterance("U2", &["Hi"]),
        utterance("U8", &["Far", "away"]),
        utterance("U9", &["Stop", "it"]),
    ];
    let pass = AdjudicationPass {
        lines: vec![
            line("U1", "Go, Lufy!"),
            line("U2", "Hi."),
            line("U8", "Far away."),
            line("U9", "Stop it."),
        ],
        findings: Findings {
            novel: vec![("U1".into(), "lufy".into()), ("U2".into(), "hey".into())],
            removed_locked: vec![("U2".into(), "Hi".into()), ("U9".into(), "it".into())],
            ..Findings::default()
        },
        ..AdjudicationPass::default()
    };
    let fix = || Chosen::FixIt {
        model: "opus".into(),
        why: "x".into(),
    };
    let corrections = Corrections {
        lines: vec![
            correction("U1", "Go, Luffy!", fix()),
            correction("U2", "Hey there.", Chosen::Typed),
            correction("U9", "Stop it now.", fix()),
        ],
    };
    let out = settled(pass, &corrections, &sheet, &[]);
    assert_eq!(out.lines[0].t, "Go, Luffy!");
    // U1's fix is heard; U9's fix has a word no engine heard near it.
    assert_eq!(out.findings.novel, [("U9".to_string(), "now".to_string())]);
    assert!(out.findings.removed_locked.is_empty());
}
