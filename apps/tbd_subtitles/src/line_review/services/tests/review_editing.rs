use job_model::outputs::Chosen;

use super::*;
use crate::line_review::services::review_loading;

fn session(name: &str) -> ReviewSession {
    let dir = std::env::temp_dir().join(format!("tbd-edit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let sheet = r#"[
        {"id":"U1","start_s":10.0,"end_s":11.0,"words":[],"locked":[],"line":"U1","hypotheses":[["P",["blame!"]],["W",["flavor!"]]]},
        {"id":"U2","start_s":12.0,"end_s":13.0,"words":[],"locked":[],"line":"U2","hypotheses":[["P",["Go!"]]]}
    ]"#;
    std::fs::write(dir.join("sheet.json"), sheet).expect("sheet");
    let adjudicated = r#"{"lines":[{"id":"U1","t":"Blaver!","f":["UNSURE","SPK"]},{"id":"U2","t":"Go!","f":[]}],
        "findings":{"missing_ids":[],"duplicate_ids":[],"unknown_ids":[],"novel":[],"removed_locked":[],"too_fast":[]},
        "calls":1,"input_tokens":0,"output_tokens":0,"cost_usd":0.0}"#;
    std::fs::write(dir.join("adjudicated.json"), adjudicated).expect("adjudicated");
    review_loading::load(Path::new("/v/a.mp4"), &dir).expect("session")
}

#[test]
fn opening_a_line_starts_from_the_settled_text_without_unsure() {
    let mut s = session("open");
    open(&mut s, "U1");
    let draft = s.draft.clone().expect("draft");
    assert_eq!(draft.text, "Blaver!");
    assert_eq!(draft.flags, ["SPK"]);
}

#[test]
fn a_picked_reading_is_saved_as_that_engines_and_written() {
    let mut s = session("pick");
    open(&mut s, "U1");
    pick(&mut s, "P");
    save(&mut s).expect("save");
    let c = s.correction("U1").expect("correction").clone();
    assert_eq!(c.text, "blame!");
    assert_eq!(c.chosen, Chosen::Engine("P".into()));
    let written: Corrections = serde_json::from_str(
        &std::fs::read_to_string(s.work_dir.join("review.json")).expect("file"),
    )
    .expect("json");
    assert_eq!(written, s.corrections);
}

#[test]
fn a_typed_text_is_typed_and_an_empty_one_is_refused_unless_dropped() {
    let mut s = session("typed");
    open(&mut s, "U1");
    if let Some(d) = &mut s.draft {
        d.text = "  Bravo,   Luffy! ".into();
    }
    save(&mut s).expect("save");
    let c = s.correction("U1").expect("correction");
    assert_eq!(c.text, "Bravo, Luffy!");
    assert_eq!(c.chosen, Chosen::Typed);
    if let Some(d) = &mut s.draft {
        d.text = String::new();
    }
    assert!(save(&mut s).is_err());
    if let Some(d) = &mut s.draft {
        d.flags.push("DROP".into());
    }
    assert!(save(&mut s).is_ok());
}

#[test]
fn reverting_the_last_correction_removes_the_file() {
    let mut s = session("revert");
    open(&mut s, "U1");
    save(&mut s).expect("save");
    assert!(s.work_dir.join("review.json").exists());
    revert(&mut s, "U1").expect("revert");
    assert!(s.correction("U1").is_none());
    assert!(!s.work_dir.join("review.json").exists());
}

#[test]
fn neighbours_follow_the_lines_shown() {
    let mut s = session("next");
    assert_eq!(neighbour(&s, "U1", true), None, "only U1 is flagged");
    s.show_all = true;
    assert_eq!(neighbour(&s, "U1", true), Some("U2".into()));
    assert_eq!(neighbour(&s, "U2", false), Some("U1".into()));
}
