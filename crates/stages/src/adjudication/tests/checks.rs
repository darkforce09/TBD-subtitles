use job_model::outputs::TimedWord;

use super::*;

fn utterance(id: &str, words: &[&str], locked: &[bool], other: &[&str]) -> Utterance {
    Utterance {
        id: id.into(),
        start_s: 0.0,
        end_s: 2.0,
        words: words
            .iter()
            .map(|t| TimedWord {
                text: t.to_string(),
                start_s: 0.0,
                end_s: 0.1,
                confidence: None,
            })
            .collect(),
        locked: locked.to_vec(),
        line: String::new(),
        hypotheses: vec![
            ("P".into(), words.iter().map(|w| w.to_string()).collect()),
            ("W".into(), other.iter().map(|w| w.to_string()).collect()),
        ],
    }
}

fn line(id: &str, t: &str, f: &[&str]) -> Line {
    Line {
        id: id.into(),
        t: t.into(),
        f: f.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn a_faithful_answer_passes() {
    let sheet = [utterance(
        "U1",
        &["the", "Birdcage", "is", "closing!"],
        &[true, false, true, true],
        &["the", "bird", "cage", "is", "closing", "in"],
    )];
    let f = check(
        &sheet,
        &[line("U1", "The birdcage is closing in!", &[])],
        &[],
    );
    assert_eq!(f, Findings::default());
}

#[test]
fn invented_words_and_dropped_agreed_words_are_caught() {
    let sheet = [utterance(
        "U1",
        &["Law", "run", "now"],
        &[true, true, true],
        &["Law", "run", "now"],
    )];
    let f = check(
        &sheet,
        &[line("U1", "Law, flee now!", &[])],
        &["Doflamingo"],
    );
    assert_eq!(f.novel, vec![("U1".to_string(), "flee".to_string())]);
    assert_eq!(
        f.removed_locked,
        vec![("U1".to_string(), "run".to_string())]
    );
}

#[test]
fn glossary_words_and_dropped_lines_are_allowed() {
    let sheet = [utterance(
        "U1",
        &["dough", "flamingo"],
        &[false, false],
        &["Doflamingo"],
    )];
    assert!(
        check(&sheet, &[line("U1", "Doflamingo!", &[])], &["Doflamingo"])
            .novel
            .is_empty()
    );
    let sheet = [utterance("U1", &["la", "la"], &[true, true], &["la", "la"])];
    assert!(
        check(&sheet, &[line("U1", "", &["LYRIC"])], &[])
            .removed_locked
            .is_empty()
    );
}

#[test]
fn ids_must_each_appear_once() {
    let sheet = [
        utterance("U1", &["a"], &[true], &["a"]),
        utterance("U2", &["b"], &[true], &["b"]),
    ];
    let f = check(
        &sheet,
        &[
            line("U1", "a", &[]),
            line("U1", "a", &[]),
            line("U9", "x", &[]),
        ],
        &[],
    );
    assert_eq!(f.missing_ids, vec!["U2"]);
    assert_eq!(f.duplicate_ids, vec!["U1"]);
    assert_eq!(f.unknown_ids, vec!["U9"]);
}

#[test]
fn joined_and_hyphenated_agreed_words_count_as_kept() {
    let sheet = [utterance(
        "U1",
        &["Don", "Quixote", "long-anticipated"],
        &[true, true, true],
        &["Don", "Quixote", "long-anticipated"],
    )];
    let f = check(
        &sheet,
        &[line("U1", "Donquixote, long-anticipated", &[])],
        &["Donquixote"],
    );
    assert!(f.removed_locked.is_empty(), "{:?}", f.removed_locked);
}
