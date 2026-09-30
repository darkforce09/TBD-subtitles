use std::io::Write as _;

use super::*;
use crate::address::{Key, Table};
use crate::frame::{Frame, read_frame};

/// Run `send` over a sink on a pipe's write end, on a thread of its own, and read every frame.
fn sent(send: impl FnOnce(&FrameSink<io::PipeWriter>) + Send + 'static) -> Vec<Frame> {
    let (mut reader, writer) = io::pipe().unwrap();
    let writing = std::thread::spawn(move || send(&FrameSink::new(writer)));
    let mut frames = Vec::new();
    while let Some(frame) = read_frame(&mut reader).unwrap() {
        frames.push(frame);
    }
    writing.join().unwrap();
    frames
}

#[test]
fn each_call_sends_its_own_frame() {
    let address = Address {
        table: Table::Outputs,
        key: Key::Name("vad".into()),
    };
    let expected = address.clone();
    let frames = sent(move |sink| {
        sink.progress(3, 80).unwrap();
        sink.model_call("{\"id\":\"1-1\"}").unwrap();
        sink.output(&expected, b"archive").unwrap();
        sink.measure(b"measure").unwrap();
        sink.failed("the step broke").unwrap();
        sink.done().unwrap();
    });
    let tags: Vec<Tag> = frames.iter().map(|frame| frame.tag).collect();
    assert_eq!(
        tags,
        [
            Tag::Progress,
            Tag::ModelCall,
            Tag::Output,
            Tag::Measure,
            Tag::Failed,
            Tag::Done
        ]
    );
    assert_eq!(
        Progress::decode(&frames[0].payload).unwrap(),
        Progress { done: 3, total: 80 }
    );
    assert_eq!(frames[1].payload, b"{\"id\":\"1-1\"}");
    let (read, taken) = Address::read(&mut frames[2].payload.as_slice()).unwrap();
    assert_eq!(read, address);
    assert_eq!(&frames[2].payload[taken..], b"archive");
    assert_eq!(frames[3].payload, b"measure");
    assert_eq!(frames[4].payload, b"the step broke");
    assert!(frames[5].payload.is_empty());
}

#[test]
fn frames_from_many_threads_stay_whole() {
    let frames = sent(|sink| {
        std::thread::scope(|scope| {
            for thread in 0..8u8 {
                scope.spawn(move || {
                    let text = vec![b'a' + thread; 5000];
                    for _ in 0..20 {
                        sink.send(Tag::Failed, &[&text[..2500], &text[2500..]])
                            .unwrap();
                    }
                });
            }
        });
    });
    assert_eq!(frames.len(), 160);
    for frame in frames {
        assert_eq!(frame.payload.len(), 5000);
        assert!(frame.payload.iter().all(|byte| *byte == frame.payload[0]));
    }
}

#[test]
fn a_closed_pipe_is_an_error_not_a_panic() {
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let sink = FrameSink::new(writer);
    assert!(sink.done().is_err());
}

#[test]
fn inputs_are_read_until_the_stream_ends() {
    let first = Address {
        table: Table::Outputs,
        key: Key::Name("probe_decode".into()),
    };
    let second = Address {
        table: Table::Frames,
        key: Key::Frame {
            occurrence: "T0001".into(),
            frame: 7,
        },
    };
    let (mut reader, mut writer) = io::pipe().unwrap();
    let addresses = [first.clone(), second.clone()];
    let writing = std::thread::spawn(move || {
        for (address, archive) in addresses.iter().zip([&b"one"[..], &b""[..]]) {
            let front = address.encode().unwrap();
            frame::write_frame(&mut writer, Tag::Input, &[&front, archive]).unwrap();
        }
        writer.flush().unwrap();
    });
    let inputs = read_inputs(&mut reader).unwrap();
    writing.join().unwrap();
    assert_eq!(inputs, [(first, b"one".to_vec()), (second, Vec::new())]);
}

#[test]
fn any_other_frame_among_the_inputs_is_invalid_data() {
    let mut bytes = Vec::new();
    frame::write_frame(&mut bytes, Tag::Progress, &[&[0; 16]]).unwrap();
    let error = read_inputs(&mut bytes.as_slice()).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn a_send_before_install_answers_false() {
    if FRAMES.get().is_none() {
        assert!(!done());
        assert!(!model_call("{}"));
    }
}
