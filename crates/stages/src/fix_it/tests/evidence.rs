use super::super::fake_model::{Fixture, GLOSSARY};
use super::*;

#[test]
fn a_block_shows_the_place_the_engines_the_line_and_the_unplaced_words() {
    let fixture = Fixture::new();
    let ep = fixture.episode(&GLOSSARY);
    let lookup = Lookup::new(&ep);
    let block = repair_block(
        &lookup,
        1,
        &["too short".to_string()],
        "Shut your filthy mouths!",
        &["SPK".to_string()],
    );
    assert!(
        block.starts_with(
            "### U0061 at 4:16.7, 1.8 s long, 4.7 s after U0060, 0.5 s before U0062\n"
        )
    );
    assert!(block.contains("Problems: too short\n"));
    assert!(block.contains("Heard P: Shut your filthy mouths!\n"));
    assert!(block.contains("Heard W: Yeah, that's right!"));
    assert!(block.contains("Now: [SPK] \"Shut your filthy mouths!\"\n"));
    assert!(block.contains("Timing: ~Yeah, ~that's ~right!"));
    assert!(block.contains(" Shut your filthy mouths!\n"));
    assert!(block.contains("Before it: U0060: \"Look at her.\"\n"));
    assert!(block.contains("After it: U0062: [SPK] \"Rebecca!\" · U0294:"));
}

#[test]
fn a_line_left_out_of_the_subtitles_says_so() {
    let fixture = Fixture::new();
    let ep = fixture.episode(&GLOSSARY);
    let lookup = Lookup::new(&ep);
    assert_eq!(lookup.timing("U0060"), "(not in the subtitles)");
    assert_eq!(shown("", &["DROP".to_string()]), "[DROP] \"(empty)\"");
}
