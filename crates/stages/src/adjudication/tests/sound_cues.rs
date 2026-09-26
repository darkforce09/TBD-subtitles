use super::*;

fn candidate(id: &str, kind: CandidateKind, start_s: f64) -> SoundCandidate {
    SoundCandidate {
        id: id.into(),
        kind,
        label: "Explosion".into(),
        start_s,
        end_s: start_s + 1.0,
        peak: 0.8,
    }
}

fn choice(id: &str, text: &str) -> Choice {
    Choice {
        id: id.into(),
        text: text.into(),
    }
}

#[test]
fn a_cue_must_be_one_bracketed_lowercase_phrase_for_a_known_candidate() {
    let all = [candidate("S001", CandidateKind::Effect, 1.0)];
    let known: HashMap<&str, &SoundCandidate> = all.iter().map(|c| (c.id.as_str(), c)).collect();
    let glossary = ["Luffy", "Trafalgar Law"];
    assert!(check_choice(&choice("S001", "[explosion]"), &known, &glossary).is_ok());
    assert!(check_choice(&choice("S001", "[Luffy laughs]"), &known, &glossary).is_ok());
    assert!(check_choice(&choice("S001", "[Law's sword clangs]"), &known, &glossary).is_ok());
    assert!(check_choice(&choice("S002", "[explosion]"), &known, &glossary).is_err());
    assert!(check_choice(&choice("S001", "explosion"), &known, &glossary).is_err());
    assert!(check_choice(&choice("S001", "[Explosion]"), &known, &glossary).is_err());
    assert!(check_choice(&choice("S001", "[boom] [crash]"), &known, &glossary).is_err());
    assert!(check_choice(&choice("S001", "[ ]"), &known, &glossary).is_err());
}

#[test]
fn settled_cues_take_candidate_times_and_every_song_gets_a_music_cue() {
    let all = [
        candidate("S001", CandidateKind::Song, 0.0),
        candidate("S002", CandidateKind::Effect, 50.0),
        candidate("S003", CandidateKind::Effect, 60.0),
        candidate("S004", CandidateKind::Song, 90.0),
    ];
    let choices = [
        choice("S003", "[door slams]"),
        choice("S003", "[door opens]"),
        choice("S004", "[upbeat music playing]"),
        choice("S009", "[thud]"),
    ];
    let result = settle(&all, &choices, &[]);
    let cues: Vec<(&str, &str, f64)> = result
        .cues
        .iter()
        .map(|c| (c.candidate.as_str(), c.text.as_str(), c.start_s))
        .collect();
    assert_eq!(
        cues,
        vec![
            ("S001", MUSIC_FALLBACK, 0.0),
            ("S003", "[door slams]", 60.0),
            ("S004", "[upbeat music playing]", 90.0)
        ]
    );
    assert_eq!(result.refused.len(), 2);
    assert_eq!(result.candidates.len(), 4);
}

#[test]
fn the_message_interleaves_candidates_with_nearby_dialogue() {
    let all = [candidate("S001", CandidateKind::Effect, 65.0)];
    let dialogue = vec![
        (10.0, "Far away.".to_string()),
        (60.0, "Look out!".to_string()),
        (70.0, "Whoa!".to_string()),
    ];
    let message = user_message(&["Luffy"], &[&all[0]], &dialogue);
    assert!(
        message
            .contains("1:00.0 Look out!\nS001 1:05.0 1.0s effect Explosion 0.80\n1:10.0 Whoa!\n"),
        "{message}"
    );
    assert!(!message.contains("Far away."));
}

#[test]
fn candidates_are_grouped_into_windows_of_video_time() {
    let all = [
        candidate("S001", CandidateKind::Effect, 10.0),
        candidate("S002", CandidateKind::Effect, 290.0),
        candidate("S003", CandidateKind::Effect, 310.0),
    ];
    let grouped = windows(&all);
    assert_eq!(grouped.iter().map(Vec::len).collect::<Vec<_>>(), vec![2, 1]);
    assert_eq!(clock(605.44), "10:05.4");
}
