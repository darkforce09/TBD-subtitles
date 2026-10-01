use std::io::{self, Write};

use super::*;

/// What a reader sees of `bytes` written on a pipe by a thread of its own.
fn through_pipe(bytes: Vec<u8>) -> io::PipeReader {
    let (reader, mut writer) = io::pipe().unwrap();
    std::thread::spawn(move || {
        writer.write_all(&bytes).unwrap();
    });
    reader
}

fn frame_bytes(tag: Tag, parts: &[&[u8]]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_frame(&mut bytes, tag, parts).unwrap();
    bytes
}

#[test]
fn every_tag_goes_through_a_pipe_unchanged() {
    let mut bytes = Vec::new();
    for (n, tag) in Tag::ALL.into_iter().enumerate() {
        let payload = vec![n as u8; n * 3];
        write_frame(&mut bytes, tag, &[&payload[..n], &payload[n..]]).unwrap();
    }
    let mut reader = through_pipe(bytes);
    for (n, tag) in Tag::ALL.into_iter().enumerate() {
        let frame = read_frame(&mut reader).unwrap().expect("a frame");
        assert_eq!(frame.tag, tag);
        assert_eq!(frame.payload, vec![n as u8; n * 3]);
    }
    assert_eq!(read_frame(&mut reader).unwrap(), None);
}

#[test]
fn a_header_is_the_tag_then_the_length_little_endian() {
    let bytes = frame_bytes(Tag::Failed, &[b"ab", b"c"]);
    assert_eq!(bytes, [6, 3, 0, 0, 0, b'a', b'b', b'c']);
}

#[test]
fn an_empty_stream_is_a_clean_end() {
    let mut reader = through_pipe(Vec::new());
    assert_eq!(read_header(&mut reader).unwrap(), None);
}

#[test]
fn a_stream_cut_inside_a_header_is_an_unexpected_end() {
    let mut reader = through_pipe(vec![Tag::Progress as u8, 16, 0]);
    let error = read_header(&mut reader).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert!(error.to_string().contains("3 of 5"), "{error}");
}

#[test]
fn a_stream_cut_inside_a_payload_is_an_unexpected_end() {
    let mut bytes = frame_bytes(Tag::ModelCall, &[b"{\"id\": \"1\"}"]);
    bytes.truncate(bytes.len() - 4);
    let mut reader = through_pipe(bytes);
    let error = read_frame(&mut reader).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    assert!(error.to_string().contains("7 of 11"), "{error}");
}

#[test]
fn an_unknown_tag_is_invalid_data() {
    let mut reader = through_pipe(vec![0x2a, 0, 0, 0, 0]);
    let error = read_header(&mut reader).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("42"), "{error}");
    assert_eq!(Tag::try_from(0), Err(0));
    assert_eq!(Tag::try_from(9), Err(9));
}

#[test]
fn a_message_frame_is_tag_eight_and_carries_its_text() {
    assert_eq!(Tag::try_from(8), Ok(Tag::Message));
    let text = "waiting for GPU memory: 4096 MiB free, 5376 needed";
    let bytes = frame_bytes(Tag::Message, &[text.as_bytes()]);
    assert_eq!(bytes[0], 8);
    let mut reader = through_pipe(bytes);
    let frame = read_frame(&mut reader).unwrap().expect("a frame");
    assert_eq!(frame.tag, Tag::Message);
    assert_eq!(frame.payload, text.as_bytes());
    assert_eq!(Tag::Message.name(), "message");
}

#[test]
fn a_payload_that_is_not_text_survives_byte_for_byte() {
    let raw = [0xff, 0xfe, 0x00, 0x0a, 0x0a, 0xc3];
    let mut reader = through_pipe(frame_bytes(Tag::Failed, &[&raw]));
    let frame = read_frame(&mut reader).unwrap().unwrap();
    assert_eq!(frame.payload, raw);
}

#[test]
fn a_frame_may_carry_no_payload() {
    let bytes = frame_bytes(Tag::Done, &[]);
    assert_eq!(bytes.len(), HEADER_LEN);
    let mut reader = through_pipe(bytes);
    let header = read_header(&mut reader).unwrap().unwrap();
    assert_eq!(
        header,
        Header {
            tag: Tag::Done,
            len: 0
        }
    );
    assert_eq!(read_payload(&mut reader, 0).unwrap(), Vec::<u8>::new());
    assert_eq!(read_header(&mut reader).unwrap(), None);
}

#[test]
fn a_false_length_claims_no_memory_the_stream_lacks() {
    let mut reader = through_pipe(vec![Tag::Output as u8, 0xff, 0xff, 0xff, 0xff, 1, 2]);
    let error = read_frame(&mut reader).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}
