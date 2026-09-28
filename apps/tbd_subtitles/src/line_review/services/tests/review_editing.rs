use job_model::outputs::Chosen;

use super::*;
use crate::job_report::models::finding_group::LineGroup;
use crate::line_review::models::session::{Hypothesis, LineList};

fn line(
    id: &str,
    start_s: f64,
    text: &str,
    flags: &[&str],
    group: Option<LineGroup>,
) -> ReviewLine {
    ReviewLine {
        id: id.into(),
        start_s,
        end_s: start_s + 1.0,
        adjudicated: text.into(),
        flags: flags.iter().map(|f| f.to_string()).collect(),
        hypotheses: vec![
            Hypothesis {
                tag: "P".into(),
                text: "blame!".into(),
            },
            Hypothesis {
                tag: "W".into(),
                text: "flavor!".into(),
            },
        ],
        groups: group.map(|g| (g, "why".to_string())).into_iter().collect(),
    }
}

/// A review in a scratch work folder: U1 unsure, U2 settled, U3 with a heard word replaced and
/// U4 too fast; the list shows the lines to check, U1 open.
fn session(name: &str) -> ReviewSession {
    let dir = std::env::temp_dir().join(format!("tbd-edit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let lines = vec![
        line(
            "U1",
            10.0,
            "Blaver!",
            &["UNSURE", "SPK"],
            Some(LineGroup::Unsure),
        ),
        line("U2", 12.0, "Go!", &[], None),
        line(
            "U3",
            14.0,
            "Frankie!",
            &[],
            Some(LineGroup::HeardWordReplaced),
        ),
        line("U4", 20.0, "Luffy!", &[], Some(LineGroup::TooFast)),
    ];
    let mut session = ReviewSession::new("/v/a.mp4".into(), dir, lines, Corrections::default());
    open(&mut session, "U1");
    session
}

fn text(s: &ReviewSession, id: &str) -> String {
    s.line(id).map(|l| s.current(l).text).unwrap_or_default()
}

#[test]
fn opening_a_line_starts_from_the_settled_text_without_unsure() {
    let s = session("open");
    let line = s.line("U1").expect("U1");
    assert_eq!(s.current(line).text, "Blaver!");
    assert_eq!(s.current(line).flags, ["SPK"]);
    assert_eq!(status(&s, line), LineStatus::ToCheck);
}

#[test]
fn a_draft_survives_switching_lines_and_goes_when_it_matches_again() {
    let mut s = session("draft");
    edit_text(&mut s, "Brave!".into());
    assert!(is_dirty(&s, "U1"));
    open(&mut s, "U3");
    set_flags(&mut s, vec!["NARR".into()]);
    open(&mut s, "U1");
    assert_eq!(text(&s, "U1"), "Brave!", "U1's edit is kept");
    assert!(is_dirty(&s, "U3"), "and U3's");
    assert_eq!(status(&s, s.line("U3").expect("U3")), LineStatus::Edited);
    edit_text(&mut s, "Blaver!".into());
    assert!(!is_dirty(&s, "U1"), "back to its text, it is no edit");
    open(&mut s, "U3");
    discard(&mut s);
    assert!(s.drafts.is_empty());
}

#[test]
fn a_picked_reading_is_saved_as_that_engines_and_the_list_moves_on() {
    let mut s = session("pick");
    pick(&mut s, "P");
    let next = save(&mut s).expect("save");
    assert_eq!(next.as_deref(), Some("U3"), "the next line to check");
    assert_eq!(s.open.as_deref(), Some("U3"));
    let c = s.correction("U1").expect("correction").clone();
    assert_eq!(c.text, "blame!");
    assert_eq!(c.flags, ["SPK"]);
    assert_eq!(c.chosen, Chosen::Engine("P".into()));
    assert!(s.drafts.is_empty());
    let written: Corrections = serde_json::from_str(
        &std::fs::read_to_string(s.work_dir.join("review.json")).expect("file"),
    )
    .expect("json");
    assert_eq!(written, s.corrections);
    s.list = LineList::All;
    open(&mut s, "U2");
    edit_text(&mut s, "Go on!".into());
    assert_eq!(
        save(&mut s).expect("save").as_deref(),
        Some("U3"),
        "every line stays listed, so the one after"
    );
}

#[test]
fn a_typed_text_is_typed_and_an_empty_one_is_refused_unless_dropped() {
    let mut s = session("typed");
    edit_text(&mut s, "  Bravo,   Luffy! ".into());
    save(&mut s).expect("save");
    let c = s.correction("U1").expect("correction");
    assert_eq!(c.text, "Bravo, Luffy!");
    assert_eq!(c.chosen, Chosen::Typed);
    assert_eq!(status(&s, s.line("U1").expect("U1")), LineStatus::Corrected);
    open(&mut s, "U3");
    edit_text(&mut s, String::new());
    let error = save(&mut s).expect_err("empty");
    assert!(error.contains("Drop the line"), "{error}");
    set_flags(&mut s, vec!["DROP".into()]);
    assert!(save(&mut s).is_ok());
}

#[test]
fn looks_right_saves_the_language_models_text_unchanged() {
    let mut s = session("keep");
    let next = looks_right(&mut s).expect("keep");
    assert_eq!(next.as_deref(), Some("U3"));
    let c = s.correction("U1").expect("correction");
    assert_eq!(c.text, "Blaver!");
    assert_eq!(c.flags, ["SPK"], "without UNSURE");
    assert_eq!(c.chosen, Chosen::Engine("adjudicated".into()));
    assert_eq!(status(&s, s.line("U1").expect("U1")), LineStatus::Kept);
}

#[test]
fn reverting_the_last_correction_removes_the_file_and_waits_for_a_run() {
    let mut s = session("revert");
    save(&mut s).expect("save");
    assert!(s.work_dir.join("review.json").exists());
    run_started(&mut s);
    run_ended(&mut s, true);
    assert_eq!(s.run("U1"), Some(RunState::Updated));
    s.list = LineList::Checked;
    open(&mut s, "U1");
    assert_eq!(revert(&mut s, "U1"), Ok(true));
    assert!(s.correction("U1").is_none());
    assert!(!s.work_dir.join("review.json").exists());
    assert_eq!(
        s.run("U1"),
        Some(RunState::Saved),
        "taken back, it is timed again"
    );
    assert_eq!(
        s.open.as_deref(),
        Some("U1"),
        "the editor stays on the line"
    );
    assert_eq!(s.list, LineList::All, "the checked list no longer shows it");
    assert_eq!(s.run_shown(), Some(RunState::Saved));
}

#[test]
fn a_saved_line_follows_its_correction_run() {
    let mut s = session("runs");
    looks_right(&mut s).expect("keep");
    assert_eq!(s.run("U1"), Some(RunState::Saved));
    assert_eq!(s.run_shown(), Some(RunState::Saved), "the newest, on U3");
    run_started(&mut s);
    assert_eq!(s.run("U1"), Some(RunState::Updating));
    looks_right(&mut s).expect("keep U3");
    assert_eq!(s.run("U3"), Some(RunState::Saved), "for the next run");
    run_ended(&mut s, true);
    assert_eq!(s.run("U1"), Some(RunState::Updated));
    assert_eq!(s.run("U3"), Some(RunState::Saved));
    run_started(&mut s);
    run_ended(&mut s, false);
    assert_eq!(s.run("U3"), Some(RunState::Failed));
    assert_eq!(s.runs.last().map(|(id, _)| id.as_str()), Some("U3"));
    run_started(&mut s);
    assert_eq!(
        s.run("U3"),
        Some(RunState::Updating),
        "the next run takes the failed line again"
    );
    let parked = park(s.clone());
    let mut fresh = session("runs-fresh");
    unpark(&mut fresh, parked);
    assert_eq!(fresh.runs, s.runs, "a closed review keeps its runs");
}

#[test]
fn nothing_is_taken_back_from_a_line_without_a_correction() {
    let mut s = session("revert-none");
    assert_eq!(revert(&mut s, "U2"), Ok(false));
    assert!(s.runs.is_empty(), "no run waits for it");
}

#[test]
fn saving_the_last_line_of_a_list_that_keeps_it_stays_on_it() {
    let mut s = session("last");
    s.list = LineList::All;
    open(&mut s, "U4");
    assert_eq!(looks_right(&mut s).expect("keep").as_deref(), Some("U4"));
    assert_eq!(s.open.as_deref(), Some("U4"));
    s.list = LineList::Checked;
    edit_text(&mut s, "Luffy, go!".into());
    assert_eq!(save(&mut s).expect("save").as_deref(), Some("U4"));
}

#[test]
fn what_the_owner_did_is_carried_over_to_the_lines_read_again() {
    let mut old = session("carry");
    edit_text(&mut old, "Brave!".into());
    open(&mut old, "U3");
    edit_text(&mut old, "Franky!".into());
    open(&mut old, "U4");
    looks_right(&mut old).expect("keep U4");
    old.list = LineList::All;
    old.search = "fr".into();
    old.group = Some(LineGroup::HeardWordReplaced);
    let mut fresh = session("carry-fresh");
    fresh.corrections = old.corrections.clone();
    fresh.corrections.set(Correction {
        id: "U3".into(),
        text: "Franky!".into(),
        flags: Vec::new(),
        chosen: Chosen::Typed,
    });
    carry_over(&old, &mut fresh);
    assert_eq!(fresh.list, LineList::All);
    assert_eq!(fresh.search, "fr");
    assert_eq!(fresh.group, Some(LineGroup::HeardWordReplaced));
    assert_eq!(fresh.runs, [("U4".to_string(), RunState::Saved)]);
    assert_eq!(fresh.open, old.open);
    assert_eq!(text(&fresh, "U1"), "Brave!", "the draft of U1 survives");
    assert!(
        !fresh.drafts.contains_key("U3"),
        "U3 has the draft's text saved now"
    );
}

/// Fix It's change of `id`, written to the work folder as Fix It writes it.
fn fixed_by_claude(s: &mut ReviewSession, id: &str, text: &str) {
    let fix = Correction {
        id: id.into(),
        text: text.into(),
        flags: Vec::new(),
        chosen: Chosen::FixIt {
            model: "opus".into(),
            why: "Both engines heard it.".into(),
        },
    };
    let work = pipeline::work_dir::WorkDir::new(&s.work_dir);
    let (corrections, ()) =
        pipeline::work_dir::update_corrections(&work, |c| c.set(fix)).expect("write");
    s.corrections = corrections;
}

#[test]
fn a_fix_it_change_waits_to_be_checked_and_keep_change_makes_it_the_owner_s() {
    let mut s = session("keep-fix");
    fixed_by_claude(&mut s, "U3", "Franky!");
    let line = s.line("U3").expect("U3").clone();
    assert_eq!(status(&s, &line), LineStatus::FixIt);
    assert!(line_filter::shown(&s).iter().any(|l| l.id == "U3"));
    open(&mut s, "U3");
    looks_right(&mut s).expect("keep");
    let kept = s.correction("U3").expect("kept");
    assert_eq!(kept.text, "Franky!");
    assert!(matches!(&kept.chosen, Chosen::KeptFixIt { model, .. } if model == "opus"));
    assert_eq!(status(&s, &line), LineStatus::Corrected);
    assert_eq!(s.run("U3"), Some(RunState::Saved));
    s.list = LineList::Checked;
    assert!(line_filter::shown(&s).iter().any(|l| l.id == "U3"));
    let _ = std::fs::remove_dir_all(&s.work_dir);
}

#[test]
fn undo_change_puts_the_language_model_s_reading_back_as_the_owner_s() {
    let mut s = session("undo-fix");
    fixed_by_claude(&mut s, "U3", "Franky!");
    open(&mut s, "U3");
    undo_change(&mut s).expect("undo");
    let undone = s.correction("U3").expect("undone");
    assert_eq!(undone.text, "Frankie!");
    assert_eq!(undone.chosen, Chosen::Engine("adjudicated".into()));
    assert!(s.corrections.by_owner("U3"));
    let _ = std::fs::remove_dir_all(&s.work_dir);
}

#[test]
fn a_save_keeps_a_fix_it_change_written_meanwhile() {
    let mut s = session("meanwhile");
    // Fix It writes to the file while the review holds the older corrections.
    let old = s.corrections.clone();
    fixed_by_claude(&mut s, "U3", "Franky!");
    s.corrections = old;
    open(&mut s, "U1");
    edit_text(&mut s, "Brave!".into());
    save(&mut s).expect("save");
    assert_eq!(s.corrections.lines.len(), 2);
    assert!(s.unchecked_fix("U3"));
    let on_disk: Corrections = serde_json::from_str(
        &std::fs::read_to_string(s.work_dir.join("review.json")).expect("file"),
    )
    .expect("json");
    assert_eq!(on_disk, s.corrections);
    let _ = std::fs::remove_dir_all(&s.work_dir);
}

#[test]
fn lines_fix_it_changed_are_marked_saved_for_their_run() {
    let mut runs = vec![("U1".to_string(), RunState::Updated)];
    mark_fixed(&mut runs, &["U1".to_string(), "U3".to_string()]);
    assert_eq!(
        runs,
        [
            ("U1".to_string(), RunState::Saved),
            ("U3".to_string(), RunState::Saved)
        ]
    );
}
