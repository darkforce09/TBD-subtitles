use super::*;

#[test]
fn progress_is_two_little_endian_counts() {
    let progress = Progress { done: 3, total: 80 };
    let bytes = progress.encode();
    assert_eq!(bytes[0], 3);
    assert_eq!(bytes[8], 80);
    assert_eq!(Progress::decode(&bytes).unwrap(), progress);
    let big = Progress {
        done: u64::MAX - 1,
        total: u64::MAX,
    };
    assert_eq!(Progress::decode(&big.encode()).unwrap(), big);
}

#[test]
fn a_payload_of_another_length_is_invalid_data() {
    for len in [0, 8, 15, 17, 24] {
        let error = Progress::decode(&vec![0; len]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{len} bytes");
    }
}
