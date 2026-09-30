use super::*;

#[test]
fn every_table_goes_through_its_name_and_its_id() {
    for table in Table::ALL {
        assert_eq!(table.name().parse::<Table>(), Ok(table));
        assert_eq!(Table::from_id(table.id()), Some(table));
    }
    assert_eq!(Table::StepRecords.to_string(), "step_records");
    assert!("steps".parse::<Table>().is_err());
    assert_eq!(Table::from_id(0), None);
}

#[test]
fn a_named_key_round_trips_and_says_how_long_it_was() {
    let address = Address {
        table: Table::Outputs,
        key: Key::Name("asr_parakeet".into()),
    };
    let mut bytes = address.encode().unwrap();
    assert_eq!(&bytes[..4], &[3, 0, 12, 0]);
    let taken = bytes.len();
    bytes.extend_from_slice(b"archive");
    let (read, len) = Address::read(&mut bytes.as_slice()).unwrap();
    assert_eq!(read, address);
    assert_eq!(len, taken);
    assert_eq!(&bytes[len..], b"archive");
}

#[test]
fn a_frame_key_round_trips_with_its_frame_number() {
    let address = Address {
        table: Table::Frames,
        key: Key::Frame {
            occurrence: "T0042 海".into(),
            frame: 1 << 40,
        },
    };
    let bytes = address.encode().unwrap();
    assert_eq!(bytes.len(), 4 + "T0042 海".len() + 8);
    let (read, len) = Address::read(&mut bytes.as_slice()).unwrap();
    assert_eq!(read, address);
    assert_eq!(len, bytes.len());
}

#[test]
fn a_bad_table_kind_or_key_is_invalid_data() {
    for bytes in [
        vec![9, 0, 0, 0],
        vec![1, 7, 0, 0],
        vec![1, 0, 2, 0, 0xff, 0xfe],
    ] {
        let error = Address::read(&mut bytes.as_slice()).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData, "{bytes:?}");
    }
    let cut = [1, 1, 1, 0, b'a', 0, 0];
    let error = Address::read(&mut cut.as_slice()).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::UnexpectedEof);
}

#[test]
fn a_key_past_the_limit_is_refused() {
    let address = Address {
        table: Table::Meta,
        key: Key::Name("k".repeat(usize::from(u16::MAX) + 1)),
    };
    assert_eq!(
        address.encode().unwrap_err().kind(),
        std::io::ErrorKind::InvalidInput
    );
}
