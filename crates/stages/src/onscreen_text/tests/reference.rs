use std::sync::atomic::{AtomicU64, Ordering};

use subtitle_formats::cue::{Cue, CueLine, FrameRate};

use super::*;

const ANCHOR: &str = "We will begin the operation when everyone is ready.";

struct Temporary(PathBuf);

impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-reference-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn event(start: &str, end: &str, text: &str) -> String {
    format!("Dialogue: 0,{start},{end},Sign,,0,0,0,,{text}\n")
}

fn dialogue(at: f64, spoken: &str) -> CueTrack {
    CueTrack {
        frame_rate: FrameRate::FILM,
        cues: vec![Cue {
            start: FrameRate::FILM.frame_round(at),
            end: FrameRate::FILM.frame_round(at + 3.0),
            lines: vec![CueLine::plain(spoken)],
            kind: CueKind::Dialogue,
        }],
    }
}

fn reference(sign: &str) -> Reference {
    Reference {
        path: PathBuf::from("verified-reference.ass"),
        lines: vec![
            parse_line(&event("0:01:40.00", "0:01:43.00", ANCHOR)).unwrap(),
            parse_line(&event("0:01:42.00", "0:01:44.00", sign)).unwrap(),
        ],
    }
}

#[test]
fn a_missing_reference_folder_or_no_folder_is_nonfatal() {
    let temporary = Temporary::new();
    assert!(load(None, None).unwrap().is_empty());
    assert!(
        load(Some(&temporary.0.join("missing")), None)
            .unwrap()
            .is_empty()
    );
    assert!(load(Some(&temporary.0), None).unwrap().is_empty());
}

#[test]
fn scene_anchor_and_independent_translation_reuse_only_the_reference_wording() {
    let reference = reference(r"{\pos(800,300)\fscx75}Dressrosa\NSOP Operation!");
    let source_start = reference.lines[1].start;
    let result = verified(
        &[reference],
        &dialogue(10.0, ANCHOR),
        12.5,
        "DRESSROSA SOP operation",
    )
    .expect("scene and reading both agree");
    assert_eq!(result.0, "Dressrosa SOP Operation!");
    assert_eq!(result.1, PathBuf::from("verified-reference.ass"));
    assert_ne!(source_start, 12.5);
    assert!(!result.0.contains("pos"));
}

#[test]
fn unrelated_readings_mismatched_edits_and_disjoint_scenes_supply_no_wording() {
    let references = [reference("Dressrosa SOP Operation")];
    assert!(
        verified(
            &references,
            &dialogue(10.0, ANCHOR),
            12.0,
            "The prison entrance"
        )
        .is_none()
    );
    assert!(
        verified(
            &references,
            &dialogue(
                10.0,
                "We cannot begin the operation until everyone is ready."
            ),
            12.0,
            "Dressrosa SOP Operation"
        )
        .is_none()
    );
    assert!(
        verified(
            &references,
            &dialogue(10.0, ANCHOR),
            30.0,
            "Dressrosa SOP Operation"
        )
        .is_none()
    );
    assert!(
        verified(
            &references,
            &dialogue(10.0, ANCHOR),
            100.0,
            "Dressrosa SOP Operation"
        )
        .is_none()
    );
}

#[test]
fn absent_short_or_non_dialogue_anchors_cannot_verify_a_scene() {
    let references = [reference("Dressrosa SOP Operation")];
    let mut no_anchor = dialogue(10.0, ANCHOR);
    no_anchor.cues.clear();
    assert!(verified(&references, &no_anchor, 12.0, "Dressrosa SOP Operation").is_none());
    assert!(
        verified(
            &references,
            &dialogue(10.0, "Let's go."),
            12.0,
            "Dressrosa SOP Operation"
        )
        .is_none()
    );
    for kind in [CueKind::Sound, CueKind::Music] {
        let mut non_dialogue = dialogue(10.0, ANCHOR);
        non_dialogue.cues[0].kind = kind;
        assert!(verified(&references, &non_dialogue, 12.0, "Dressrosa SOP Operation").is_none());
    }
    for invalid in [f64::NAN, f64::INFINITY, -1.0] {
        assert!(
            verified(
                &references,
                &dialogue(10.0, ANCHOR),
                invalid,
                "Dressrosa SOP Operation"
            )
            .is_none()
        );
    }
    assert!(verified(&references, &dialogue(10.0, ANCHOR), 12.0, "...!?").is_none());
}

#[test]
fn loading_excludes_the_jobs_own_output_and_ignores_other_file_types() {
    let temporary = Temporary::new();
    let own = temporary.0.join("episode.ass");
    let other = temporary.0.join("reference.ASS");
    let text = event("0:00:01.00", "0:00:02.00", "Fated Reunion");
    for file in [&own, &other, &temporary.0.join("not-ass.txt")] {
        std::fs::write(file, &text).unwrap();
    }
    let references = load(Some(&temporary.0), Some(&own)).unwrap();
    assert_eq!(references.len(), 1);
    assert_eq!(references[0].path, other);
    assert_eq!(references[0].lines[0].text, "Fated Reunion");
}

#[cfg(unix)]
#[test]
fn a_symlink_to_the_jobs_output_is_excluded_too() {
    let temporary = Temporary::new();
    let own = temporary.0.join("episode.ass");
    std::fs::write(&own, event("0:00:01.00", "0:00:02.00", "Fated Reunion")).unwrap();
    std::os::unix::fs::symlink(&own, temporary.0.join("alias.ass")).unwrap();
    assert!(load(Some(&temporary.0), Some(&own)).unwrap().is_empty());
}

#[test]
fn every_positive_drawing_mode_is_ignored_while_position_tags_and_p_zero_keep_text() {
    for mode in ["1", "2", "3", "10", "001", "999999999999999999999999999"] {
        let text = format!(r"{{\p{mode}}}m 0 0 l 10 0 10 10{{\p0}}Readable ending");
        assert!(
            parse_line(&event("0:00:01.00", "0:00:02.00", &text)).is_none(),
            "{mode}"
        );
    }
    let line = parse_line(&event(
        "0:00:01.00",
        "0:00:02.00",
        r"{\pos(20,30)\pbo2\p0}Fated\hReunion\NRebecca, the gladiator",
    ))
    .unwrap();
    assert_eq!(line.text, "Fated Reunion Rebecca, the gladiator");
}

#[test]
fn malformed_times_reversed_ranges_and_broken_overrides_are_ignored() {
    assert_eq!(time("1:02:03.45"), Some(3723.45));
    for invalid in [
        "",
        "1:02",
        "1:02:03:04",
        "-1:02:03",
        "0:-1:70",
        "0:60:00",
        "0:00:60",
        "0:00:NaN",
        "0:00:1e2",
        "0:00:.5",
        "0:00:01.",
    ] {
        assert!(time(invalid).is_none(), "{invalid}");
        assert!(parse_line(&event(invalid, "1:00:00.00", "Operation")).is_none());
    }
    assert!(parse_line(&event("0:00:03.00", "0:00:02.00", "Operation")).is_none());
    assert!(parse_line(&event("0:00:03.00", "0:00:03.00", "Operation")).is_none());
    for text in [
        r"{\pos(1,2)Operation",
        r"Operation}",
        r"{{\b1}}Operation",
        "...!?",
    ] {
        assert!(
            parse_line(&event("0:00:01.00", "0:00:02.00", text)).is_none(),
            "{text}"
        );
    }
    assert!(parse_line("Comment: 0,0:00:01.00,0:00:02.00,Sign,,0,0,0,,Operation").is_none());
    assert!(parse_line("Dialogue: incomplete").is_none());
}

#[test]
fn file_event_order_does_not_change_scene_matching() {
    let mut reference = reference("Dressrosa SOP Operation");
    reference.lines.reverse();
    assert!(
        verified(
            &[reference],
            &dialogue(10.0, ANCHOR),
            12.0,
            "Dressrosa SOP Operation"
        )
        .is_some()
    );
}
