use super::*;

/// SEI (6), SPS (7), PPS (8) and an IDR slice (5), each after a four-byte start code but the PPS,
/// which follows a three-byte one.
const ANNEX_B_IDR: [u8; 22] = [
    0, 0, 0, 1, 0x06, 0x05, 0x80, //
    0, 0, 0, 1, 0x67, 0x4D, //
    0, 0, 1, 0x68, 0xEE, //
    0, 0, 1, 0x65,
];

#[test]
fn annex_b_units_are_found_after_three_and_four_byte_start_codes() {
    assert_eq!(
        nal_unit_types(&ANNEX_B_IDR, PacketFormat::AnnexB),
        Some(vec![6, 7, 8, 5])
    );
    assert!(starts_idr_picture(&ANNEX_B_IDR, PacketFormat::AnnexB));
}

#[test]
fn a_non_idr_slice_first_means_no_idr_picture() {
    let recovery_point = [0, 0, 1, 0x06, 0x06, 0x80, 0, 0, 1, 0x41, 0x9A];
    assert_eq!(
        nal_unit_types(&recovery_point, PacketFormat::AnnexB),
        Some(vec![6, 1])
    );
    assert!(!starts_idr_picture(&recovery_point, PacketFormat::AnnexB));
    let headers_only = [0, 0, 1, 0x67, 0x64, 0, 0, 1, 0x68, 0xEE];
    assert!(!starts_idr_picture(&headers_only, PacketFormat::AnnexB));
}

#[test]
fn length_prefixed_units_follow_their_lengths() {
    // An SEI of 3 bytes and an IDR slice of 2 bytes with 4-byte lengths.
    let packet = [0, 0, 0, 3, 0x06, 0x05, 0x80, 0, 0, 0, 2, 0x65, 0x88];
    assert_eq!(
        nal_unit_types(&packet, PacketFormat::LengthPrefixed(4)),
        Some(vec![6, 5])
    );
    assert!(starts_idr_picture(&packet, PacketFormat::LengthPrefixed(4)));
    // The same units with 2-byte lengths, and a non-IDR slice with 1-byte lengths.
    let short = [0, 3, 0x06, 0x05, 0x80, 0, 2, 0x65, 0x88];
    assert!(starts_idr_picture(&short, PacketFormat::LengthPrefixed(2)));
    let tiny = [2, 0x41, 0x9A];
    assert_eq!(
        nal_unit_types(&tiny, PacketFormat::LengthPrefixed(1)),
        Some(vec![1])
    );
    assert!(!starts_idr_picture(&tiny, PacketFormat::LengthPrefixed(1)));
}

#[test]
fn the_packet_format_decides_how_units_are_read() {
    // A one-byte IDR unit after a 4-byte length; with a length of 9 the bytes hold no start code.
    let packet = [0, 0, 0, 1, 0x65];
    assert_eq!(
        nal_unit_types(&packet, PacketFormat::LengthPrefixed(4)),
        Some(vec![5])
    );
    assert!(!starts_idr_picture(
        &[0, 0, 0, 9, 0x65],
        PacketFormat::AnnexB
    ));
}

#[test]
fn malformed_packets_have_no_units() {
    let formats = [PacketFormat::AnnexB, PacketFormat::LengthPrefixed(4)];
    for format in formats {
        assert_eq!(nal_unit_types(&[], format), None);
    }
    // Data before the first start code, and a start code with nothing after it.
    assert_eq!(
        nal_unit_types(&[7, 0, 0, 1, 0x65], PacketFormat::AnnexB),
        None
    );
    assert_eq!(nal_unit_types(&[0, 0, 1], PacketFormat::AnnexB), None);
    // A forbidden bit, a length past the end, an empty unit, an impossible prefix size.
    assert_eq!(nal_unit_types(&[0, 0, 1, 0xE5], PacketFormat::AnnexB), None);
    let past_end = [0, 0, 0, 9, 0x65, 0x88];
    assert_eq!(
        nal_unit_types(&past_end, PacketFormat::LengthPrefixed(4)),
        None
    );
    let empty = [0, 0, 0, 0, 0, 0, 0, 1, 0x65];
    assert_eq!(
        nal_unit_types(&empty, PacketFormat::LengthPrefixed(4)),
        None
    );
    assert_eq!(
        nal_unit_types(&[1, 0x65], PacketFormat::LengthPrefixed(5)),
        None
    );
    assert!(!starts_idr_picture(
        &past_end,
        PacketFormat::LengthPrefixed(4)
    ));
}

#[test]
fn an_ffprobe_hex_dump_reads_back_its_bytes() {
    let dump = [
        "00000000: 0000 14f7 6588 8200 5f44 222a 60c0 85f8  ....e..._D\"*`...",
        "00000010: 0002 1da0 0021 7600 0208 f0              .....!v....",
    ];
    let bytes = parse_hex_dump(dump).unwrap();
    assert_eq!(bytes.len(), 27);
    assert_eq!(&bytes[..5], &[0x00, 0x00, 0x14, 0xF7, 0x65]);
    assert_eq!(bytes[26], 0xF0);
    // Printable text that looks like hex never counts.
    let text = dump_line(0, "6588 ", "ab12 cd34");
    assert_eq!(parse_hex_dump([text.as_str()]), Some(vec![0x65, 0x88]));
    assert_eq!(parse_hex_dump([]), Some(Vec::new()));
}

/// One hex dump line as ffprobe lays it out: the hex columns padded to 41 characters.
fn dump_line(offset: usize, hex: &str, text: &str) -> String {
    format!("{offset:08x}: {hex:<41}{text}")
}

#[test]
fn a_hex_dump_out_of_sequence_or_malformed_is_refused() {
    let skipped = dump_line(16, "6588 ", "e.");
    assert_eq!(parse_hex_dump([skipped.as_str()]), None);
    assert_eq!(parse_hex_dump(["0000000: 65"]), None);
    let odd = dump_line(0, "658 ", "e.");
    assert_eq!(parse_hex_dump([odd.as_str()]), None);
    let not_hex = dump_line(0, "zz88 ", "..");
    assert_eq!(parse_hex_dump([not_hex.as_str()]), None);
    let empty = dump_line(0, "", "");
    assert_eq!(parse_hex_dump([empty.as_str()]), None);
    let too_long = dump_line(0, "0011 2233 4455 6677 8899 aabb ccdd eeff 0011", "");
    assert_eq!(parse_hex_dump([too_long.as_str()]), None);
    assert_eq!(parse_hex_dump(["pts_time=1.0"]), None);
}
