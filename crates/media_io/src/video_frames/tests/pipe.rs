use super::*;

#[test]
fn the_wide_pipe_never_exceeds_one_mebibyte_or_the_system_limit() {
    assert_eq!(clamp_to_max(None), WIDE_PIPE_BYTES);
    assert_eq!(clamp_to_max(Some("1048576\n")), WIDE_PIPE_BYTES);
    assert_eq!(clamp_to_max(Some("4194304")), WIDE_PIPE_BYTES);
    assert_eq!(clamp_to_max(Some("262144\n")), 262_144);
    assert_eq!(
        clamp_to_max(Some("0")),
        WIDE_PIPE_BYTES,
        "a zero limit is unreadable"
    );
    assert_eq!(clamp_to_max(Some("many")), WIDE_PIPE_BYTES);
    let size = wide_pipe_bytes();
    assert!(size > 0 && size <= WIDE_PIPE_BYTES);
}

#[test]
fn a_pipe_grows_to_the_wide_size() {
    let (reader, _writer) = std::io::pipe().unwrap();
    let before = capacity(&reader).unwrap();
    assert!(before > 0);
    let after = enlarge(&reader).unwrap();
    assert!(after >= before);
}
