use std::sync::atomic::AtomicBool;

use job_model::outputs::{Chosen, Correction, FixFamily, FixVerdict};
use job_model::report::QcCheck;
use serde_json::Value;

use super::fake_model::{
    Fixture, GLOSSARY, asked_ids, brief_answer, finding, judge_answer, repair_answer, scripted,
};
use super::*;

/// Which pass a call belongs to, by its system prompt.
fn family_of(system: &str) -> Option<FixFamily> {
    FixFamily::ALL
        .into_iter()
        .find(|f| system == prompt::repair(*f))
}

fn is_brief(system: &str) -> bool {
    system.starts_with(prompt::BRIEF)
}

/// The Dressrosa 12 fixes: the crowd line Whisper alone heard goes, the heard "Uh" comes back,
/// and "Oh?" stays as it is to be timed again; the judge accepts every change.
fn dressrosa_script(system: &str, user: &str) -> Result<Value, String> {
    if is_brief(system) {
        return Ok(brief_answer());
    }
    if system == prompt::JUDGE {
        return Ok(judge_answer(user, |_| Some(true)));
    }
    Ok(repair_answer(user, |id| match id {
        "U0061" => Some(("Shut your filthy mouths!", vec!["SPK"])),
        "U0295" => Some((
            "Uh, does that mean you're gonna stay in this country?",
            vec!["SPK"],
        )),
        _ => None,
    }))
}

fn line<'a>(run: &'a FixRun, id: &str) -> &'a LineFix {
    run.lines.iter().find(|l| l.id == id).expect(id)
}

#[test]
fn the_brief_reads_the_names_the_glossary_and_every_line() {
    let fixture = Fixture::new();
    let (make, calls) = scripted(dressrosa_script);
    run(
        &fixture.episode(&GLOSSARY),
        &*make,
        2,
        &|_, _, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    let calls = calls.lock().unwrap();
    let brief = calls.iter().find(|c| is_brief(&c.system)).unwrap();
    assert!(brief.user.contains("Video file: [Muhn Pace] Dressrosa 12"));
    assert!(brief.user.contains("Folder: one_pace"));
    assert!(
        brief
            .user
            .contains("Glossary (one_piece): Rebecca, Violet, Colosseum")
    );
    for l in &fixture.lines {
        assert!(brief.user.contains(&format!("{} ", l.id)), "{}", l.id);
    }
    let repair = calls
        .iter()
        .find(|c| family_of(&c.system).is_some())
        .unwrap();
    assert!(
        repair
            .user
            .starts_with("Brief\nVideo file: [Muhn Pace] Dressrosa 12")
    );
    assert!(repair.user.contains("Show: One Piece (anime), English dub"));
}

#[test]
fn the_three_dressrosa_cases_come_out_as_the_scene_needs() {
    let fixture = Fixture::new();
    let (make, calls) = scripted(dressrosa_script);
    let result = run(
        &fixture.episode(&GLOSSARY),
        &*make,
        4,
        &|_, _, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    let crowd = line(&result, "U0061");
    assert_eq!(crowd.after_text, "Shut your filthy mouths!");
    assert!(matches!(crowd.verdict, FixVerdict::Accepted { .. }));
    assert_eq!(crowd.removed.len(), 9, "{:?}", crowd.removed);
    let question = line(&result, "U0295");
    assert!(question.after_text.starts_with("Uh, does"));
    assert!(question.problems[0].contains("the main engine heard there: \"Uh\""));
    assert!(matches!(question.verdict, FixVerdict::Accepted { .. }));
    let oh = line(&result, "U0314");
    assert_eq!(oh.after_text, "Oh?");
    assert!(
        matches!(&oh.verdict, FixVerdict::Kept { why } if why.starts_with("Timed again alone.")),
        "{:?}",
        oh.verdict
    );
    // One brief, one timing repair (the only family with lines), one judge call.
    assert_eq!(calls.lock().unwrap().len(), 3);
    assert_eq!(result.usage.calls, 3);
    assert!((result.usage.cost_usd - 0.03).abs() < 1e-9);
}

#[test]
fn a_word_no_engine_heard_is_refused_and_the_line_stays() {
    let mut fixture = Fixture::new();
    fixture.qc.findings = vec![finding(
        QcCheck::Novel,
        "U0062",
        259.0,
        "Rebecca!",
        "U0062: Rebeca",
    )];
    let (make, _) = scripted(|system, user| {
        if is_brief(system) {
            return Ok(brief_answer());
        }
        Ok(repair_answer(user, |_| {
            Some(("Princess Rebecca!", vec!["SPK"]))
        }))
    });
    let result = run(
        &fixture.episode(&GLOSSARY),
        &*make,
        1,
        &|_, _, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    let rebecca = line(&result, "U0062");
    assert_eq!(rebecca.after_text, "Rebecca!");
    assert_eq!(rebecca.verdict, FixVerdict::Unchanged);
    assert!(
        rebecca.refused[0].contains("\"princess\""),
        "{:?}",
        rebecca.refused
    );
}

#[test]
fn only_the_judge_s_accepted_changes_are_kept() {
    let fixture = Fixture::new();
    let (make, _) = scripted(|system, user| {
        if is_brief(system) {
            return Ok(brief_answer());
        }
        if system == prompt::JUDGE {
            return Ok(judge_answer(user, |id| match id {
                "U0061" => Some(false),
                _ => None,
            }));
        }
        dressrosa_script(system, user)
    });
    let result = run(
        &fixture.episode(&GLOSSARY),
        &*make,
        1,
        &|_, _, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(matches!(
        line(&result, "U0061").verdict,
        FixVerdict::TurnedDown { .. }
    ));
    assert!(matches!(
        line(&result, "U0295").verdict,
        FixVerdict::NotJudged { .. }
    ));
}

#[test]
fn a_failed_repair_call_leaves_its_lines_as_they_were_and_not_answered() {
    let fixture = Fixture::new();
    let (make, calls) = scripted(|system, _| {
        if is_brief(system) {
            return Ok(brief_answer());
        }
        Err("rate limited".into())
    });
    let result = run(
        &fixture.episode(&GLOSSARY),
        &*make,
        2,
        &|_, _, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(!result.lines.is_empty());
    for line in &result.lines {
        assert_eq!(line.after_text, line.before_text);
        assert!(
            matches!(&line.verdict, FixVerdict::NotAnswered { why } if why.contains("rate limited")),
            "{}: {:?}",
            line.id,
            line.verdict
        );
        assert!(!line.verdict.answered());
    }
    assert!(result.usage.failed[0].contains("rate limited"));
    // Nothing changed, so the judge is not called.
    assert_eq!(calls.lock().unwrap().len(), 2);
}

#[test]
fn an_unreadable_answer_leaves_its_lines_not_answered() {
    let fixture = Fixture::new();
    let (make, _) = scripted(|system, _| {
        if is_brief(system) {
            return Ok(brief_answer());
        }
        Ok(serde_json::json!({"verdicts": []}))
    });
    let result = run(
        &fixture.episode(&GLOSSARY),
        &*make,
        1,
        &|_, _, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(result.lines.iter().all(|l| matches!(
        &l.verdict,
        FixVerdict::NotAnswered { why } if why.contains("does not match the schema")
    )));
}

#[test]
fn an_asked_line_records_the_checks_it_was_asked_about() {
    let fixture = Fixture::new();
    let (make, _) = scripted(dressrosa_script);
    let result = run(
        &fixture.episode(&GLOSSARY),
        &*make,
        1,
        &|_, _, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(line(&result, "U0061").checks, [QcCheck::TooShort]);
    assert_eq!(line(&result, "U0295").checks, [QcCheck::UncoveredSpeech]);
    assert!(result.lines.iter().all(|l| l.verdict.answered()));
}

#[test]
fn a_line_the_owner_settled_is_never_asked_about() {
    let mut fixture = Fixture::new();
    fixture.corrections.set(Correction {
        id: "U0061".into(),
        text: "Shut your filthy mouths!".into(),
        flags: vec!["SPK".into()],
        chosen: Chosen::Typed,
    });
    let (make, calls) = scripted(dressrosa_script);
    let result = run(
        &fixture.episode(&GLOSSARY),
        &*make,
        1,
        &|_, _, _| {},
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(result.lines.iter().all(|l| l.id != "U0061"));
    let calls = calls.lock().unwrap();
    assert!(
        calls
            .iter()
            .all(|c| !asked_ids(&c.user).contains(&"U0061".to_string()))
    );
}

#[test]
fn a_failed_brief_ends_the_run_before_any_line_is_asked() {
    let fixture = Fixture::new();
    let (make, calls) = scripted(|_, _| Err("not logged in".into()));
    let result = run(
        &fixture.episode(&GLOSSARY),
        &*make,
        1,
        &|_, _, _| {},
        &AtomicBool::new(false),
    );
    assert!(matches!(result, Err(FixFailure::Brief(_))));
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn a_stopped_run_makes_no_further_call() {
    let fixture = Fixture::new();
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let setter = stop.clone();
    let (make, calls) = scripted(move |system, user| {
        setter.store(true, std::sync::atomic::Ordering::SeqCst);
        dressrosa_script(system, user)
    });
    let result = run(&fixture.episode(&GLOSSARY), &*make, 1, &|_, _, _| {}, &stop);
    assert_eq!(result.unwrap_err(), FixFailure::Stopped);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn progress_names_each_pass_in_turn() {
    let fixture = Fixture::new();
    let (make, _) = scripted(dressrosa_script);
    let seen = std::sync::Mutex::new(Vec::new());
    run(
        &fixture.episode(&GLOSSARY),
        &*make,
        1,
        &|pass, done, total| seen.lock().unwrap().push((pass, done, total)),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(
        seen.into_inner().unwrap(),
        vec![
            (Pass::Reading, 1, 1),
            (Pass::Fixing(FixFamily::Timing), 1, 1),
            (Pass::Checking, 1, 1),
        ]
    );
}
