use super::*;
use crate::layout::FROZEN_FOLDERS;

#[test]
fn documentation_root_markdown_is_live_unless_it_is_a_frozen_record() {
    let root = DOCUMENTATION_ROOT;
    assert_eq!(
        judged_area(&format!("{root}/runbooks/deploy.md")),
        Some(DocumentArea::LiveDocumentation)
    );
    assert_eq!(
        judged_area(&format!("{root}/README.md")),
        Some(DocumentArea::LiveDocumentation)
    );
    let frozen = FROZEN_FOLDERS[0];
    assert_eq!(
        judged_area(&format!("{frozen}/stack.MD")),
        Some(DocumentArea::FrozenDocumentation)
    );
    assert_eq!(
        judged_area(&format!("{frozen}/README.md")),
        Some(DocumentArea::LiveDocumentation),
        "a frozen folder's index stays live"
    );
    assert_eq!(judged_area(&format!("{root}/charts/data.json")), None);
}

#[test]
fn readmes_anywhere_and_the_project_instructions_are_judged() {
    assert_eq!(
        judged_area(PROJECT_INSTRUCTIONS),
        Some(DocumentArea::ProjectInstructions)
    );
    assert_eq!(judged_area("README.md"), Some(DocumentArea::Readmes));
    assert_eq!(
        judged_area("crates/media_io/src/README.md"),
        Some(DocumentArea::Readmes)
    );
    assert_eq!(judged_area("crates/media_io/notes.md"), None);
    assert_eq!(judged_area("crates/media_io/src/lib.rs"), None);
}

#[test]
fn only_the_frozen_area_is_frozen() {
    for area in DocumentArea::ALL {
        assert_eq!(area.is_frozen(), area == DocumentArea::FrozenDocumentation);
    }
}
