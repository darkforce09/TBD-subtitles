use job_model::outputs::{FixBrief, Suspect};

use super::super::fake_model::{Fixture, GLOSSARY};
use super::*;

fn part(show: &str, cast: &[&str], summary: &str, suspects: &[&str]) -> FixBrief {
    FixBrief {
        show: show.into(),
        episode: String::new(),
        cast: cast.iter().map(|s| s.to_string()).collect(),
        summary: summary.into(),
        speech_habits: vec!["Luffy repeats \"meat\"".into()],
        suspects: suspects
            .iter()
            .map(|id| Suspect {
                id: id.to_string(),
                why: "odd".into(),
            })
            .collect(),
    }
}

#[test]
fn every_line_is_listed_with_its_time_and_flags() {
    let fixture = Fixture::new();
    let lines = line_list(&fixture.episode(&GLOSSARY));
    assert_eq!(lines.len(), fixture.lines.len());
    assert_eq!(lines[0], "U0060 4:10.0 | Look at her.");
    assert_eq!(lines[7], "U0314 19:16.3 [SPK] | Oh?");
}

#[test]
fn a_later_part_carries_the_summaries_before_it() {
    let fixture = Fixture::new();
    let ep = fixture.episode(&GLOSSARY);
    let text = message(
        &ep,
        &["U0314 19:16.3 | Oh?".to_string()],
        &["Part one.".to_string()],
    );
    assert!(text.starts_with("Video file: [Muhn Pace] Dressrosa 12\nFolder: one_pace\n"));
    assert!(text.contains("Summary of part 1: Part one."));
    assert!(text.ends_with("Lines:\nU0314 19:16.3 | Oh?\n"));
}

#[test]
fn parts_merge_into_one_brief_with_known_suspects_only() {
    let fixture = Fixture::new();
    let ep = fixture.episode(&GLOSSARY);
    let merged = merge(
        &ep,
        vec![
            part(
                "One Piece",
                &["Luffy", "Rebecca"],
                "First.",
                &["U0061", "U9999"],
            ),
            part("", &["Rebecca", "Sanji"], "Second.", &["U0061", "U0314"]),
        ],
    );
    assert_eq!(merged.show, "One Piece");
    assert_eq!(merged.cast, ["Luffy", "Rebecca", "Sanji"]);
    assert_eq!(merged.summary, "First.\n\nSecond.");
    assert_eq!(merged.speech_habits.len(), 1);
    let ids: Vec<&str> = merged.suspects.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["U0061", "U0314"]);
}

#[test]
fn the_context_opens_with_the_video_and_holds_the_whole_brief() {
    let fixture = Fixture::new();
    let ep = fixture.episode(&GLOSSARY);
    let text = context(&ep, &part("One Piece", &["Luffy"], "Colosseum.", &[]));
    assert!(text.starts_with("Brief\nVideo file: [Muhn Pace] Dressrosa 12 (folder one_pace)\n"));
    assert!(text.contains("Cast: Luffy\n"));
    assert!(text.contains("Speech habits: Luffy repeats \"meat\"\n"));
    assert!(text.contains("Scenes: Colosseum.\n"));
    assert!(text.ends_with("Glossary (one_piece): Rebecca, Violet, Colosseum\n"));
}
