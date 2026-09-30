use super::*;

#[test]
fn an_opened_outcome_renders_its_time() {
    assert_eq!(
        OpenOutcome::Opened { ms: 1.25 }.render(),
        "opened in 1.250 ms"
    );
}

#[test]
fn a_failed_outcome_renders_display_and_debug() {
    let outcome = OpenOutcome::failed(0.04, &DatabaseError::DatabaseAlreadyOpen);
    assert_eq!(
        outcome.render(),
        "failed in 0.040 ms: Database already open. Cannot acquire lock. | DatabaseAlreadyOpen"
    );
    assert_eq!(
        outcome.error_text().as_deref(),
        Some("Database already open. Cannot acquire lock. | DatabaseAlreadyOpen")
    );
}

#[test]
fn names_match_the_command_line() {
    assert_eq!(OpenMode::ReadWrite.name(), "rw");
    assert_eq!(OpenMode::ReadOnly.name(), "ro");
    for sharing in [
        Sharing::Exclusive,
        Sharing::SingleWriter,
        Sharing::MultiWriter,
    ] {
        let parsed = Sharing::from_str(sharing.name(), false).expect("the name parses");
        assert_eq!(parsed, sharing);
    }
}

#[test]
fn the_build_default_sharing_resolves() {
    assert_eq!(
        Sharing::resolve(None).expect("the default resolves"),
        Sharing::build_default()
    );
    assert!(Sharing::resolve(Some(Sharing::Exclusive)).is_ok());
    assert_eq!(
        Sharing::resolve(Some(Sharing::MultiWriter)).is_ok(),
        cfg!(feature = "multiprocess")
    );
}
