use job_model::outputs::{Chosen, Correction, Corrections};

use super::*;
use crate::job_report::models::finding_group::LineGroup;

fn line(id: &str, start_s: f64, text: &str, group: Option<LineGroup>) -> ReviewLine {
    ReviewLine {
        id: id.into(),
        start_s,
        end_s: start_s + 1.5,
        adjudicated: text.into(),
        flags: Vec::new(),
        hypotheses: Vec::new(),
        groups: group.map(|g| (g, "why".to_string())).into_iter().collect(),
    }
}

/// U1 unsure and corrected, U2 settled, U3 with a heard word replaced, U4 too fast.
fn session() -> ReviewSession {
    let lines = vec![
        line("U1", 10.0, "Blaver!", Some(LineGroup::Unsure)),
        line("U2", 12.0, "Go on, Luffy!", None),
        line("U3", 14.0, "Frankie!", Some(LineGroup::HeardWordReplaced)),
        line("U4", 20.0, "Luffy, run!", Some(LineGroup::TooFast)),
    ];
    let mut corrections = Corrections::default();
    corrections.set(Correction {
        id: "U1".into(),
        text: "Brave!".into(),
        flags: Vec::new(),
        chosen: Chosen::Typed,
    });
    ReviewSession::new("/v/a.mp4".into(), "/w".into(), lines, corrections)
}

fn ids(session: &ReviewSession) -> Vec<&str> {
    shown(session).iter().map(|line| line.id.as_str()).collect()
}

#[test]
fn the_lists_hold_the_lines_to_check_the_checked_ones_and_every_line() {
    let mut s = session();
    assert_eq!(
        counts(&s),
        Counts {
            to_check: 2,
            checked: 1,
            all: 4
        }
    );
    assert_eq!(ids(&s), ["U3", "U4"]);
    s.list = LineList::Checked;
    assert_eq!(ids(&s), ["U1"]);
    s.list = LineList::All;
    assert_eq!(ids(&s), ["U1", "U2", "U3", "U4"]);
    s.group = Some(LineGroup::HeardWordReplaced);
    assert_eq!(ids(&s), ["U3"], "narrowed to the group");
    assert_eq!(counts(&s).all, 4, "the counts ignore the group");
}

#[test]
fn a_search_finds_words_ids_and_times() {
    let mut s = session();
    s.list = LineList::All;
    s.search = "LUFFY".into();
    assert_eq!(ids(&s), ["U2", "U4"]);
    s.search = "brave".into();
    assert_eq!(ids(&s), ["U1"], "the corrected text is searched");
    s.search = "u3".into();
    assert_eq!(ids(&s), ["U3"]);
    s.search = "0:14".into();
    assert_eq!(ids(&s), ["U3"], "said during 0:14");
    s.search = "0:1".into();
    assert_eq!(ids(&s), ["U1", "U2", "U3"], "said from 0:10 to 0:20");
    s.search = "nobody".into();
    assert!(ids(&s).is_empty());
}

#[test]
fn a_typed_time_names_a_span_of_the_video() {
    assert_eq!(parse_time("16:33"), Some((993.0, 1.0)));
    let (at, step) = parse_time("16:33.4").expect("tenths");
    assert!((at - 993.4).abs() < 1e-9 && (step - 0.1).abs() < 1e-9);
    assert_eq!(parse_time("16:3"), Some((990.0, 10.0)));
    assert_eq!(parse_time("16:"), Some((960.0, 60.0)));
    assert_eq!(parse_time("1:02:05"), Some((3725.0, 1.0)));
    for no_time in [
        "luffy", "16", "16:7", "16:75", "1:2:05", "1:62:05", "a:10", "16:33.x",
    ] {
        assert_eq!(parse_time(no_time), None, "{no_time}");
    }
}

#[test]
fn the_editor_shows_the_open_line_while_it_is_listed_and_steps_through_the_list() {
    let mut s = session();
    assert_eq!(
        open_line(&s).map(|l| l.id.as_str()),
        Some("U3"),
        "the first"
    );
    s.open = Some("U4".into());
    assert_eq!(open_line(&s).map(|l| l.id.as_str()), Some("U4"));
    assert_eq!(neighbour(&s, false).as_deref(), Some("U3"));
    assert_eq!(neighbour(&s, true), None, "the last line");
    s.open = Some("U2".into());
    assert_eq!(
        open_line(&s).map(|l| l.id.as_str()),
        Some("U3"),
        "U2 is not to check"
    );
    s.list = LineList::Checked;
    s.search = "zzz".into();
    assert!(open_line(&s).is_none());
}

#[test]
fn a_time_too_large_to_count_is_no_time() {
    assert_eq!(parse_time("4294967295:00"), None);
    assert_eq!(parse_time("4294967295:59:00"), None);
    assert_eq!(parse_time("99999999999999:00"), None);
    let mut s = session();
    s.list = LineList::All;
    s.search = "4294967295:00".into();
    assert!(ids(&s).is_empty(), "no match, and no panic");
}

#[test]
fn a_line_stays_worth_a_listen_after_its_run_settles_it_and_while_taken_back() {
    use crate::line_review::models::session::RunState;
    let mut s = session();
    // The correction run of U1 dropped its finding.
    s.lines[0].groups.clear();
    assert_eq!(worth(&s), 3, "U1 corrected, U3 and U4 flagged");
    assert_eq!(counts(&s).checked, 1);
    // U1 taken back: no correction, its run not ended yet.
    s.corrections = Corrections::default();
    s.runs = vec![("U1".into(), RunState::Updating)];
    assert_eq!(ids(&s), ["U1", "U3", "U4"]);
    assert_eq!(worth(&s), 3);
    s.runs = vec![("U1".into(), RunState::Updated)];
    assert_eq!(
        ids(&s),
        ["U3", "U4"],
        "once its run ends, the check decides"
    );
}
