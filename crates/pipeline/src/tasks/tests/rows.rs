use worker_channel::address::Address;
use worker_channel::frame::{Tag, write_frame};

use super::*;

fn row(table: Table, occurrence: &str, frame: u64) -> Input {
    (
        Address {
            table,
            key: Key::Frame {
                occurrence: occurrence.into(),
                frame,
            },
        },
        format!("{occurrence}/{frame}").into_bytes(),
    )
}

fn piped(rows: &[Input]) -> Box<dyn Read + Send> {
    let mut bytes = Vec::new();
    for (address, archive) in rows {
        write_frame(
            &mut bytes,
            Tag::Input,
            &[&address.encode().unwrap(), archive],
        )
        .unwrap();
    }
    Box::new(std::io::Cursor::new(bytes))
}

fn read(stream: &mut RowStream, table: Table) -> Result<Vec<String>> {
    let mut seen = Vec::new();
    stream.each(table, |occurrence, frame, bytes| {
        assert_eq!(bytes, format!("{occurrence}/{frame}").as_bytes());
        seen.push(format!("{occurrence}/{frame}"));
        Ok(())
    })?;
    Ok(seen)
}

#[test]
fn rows_arrive_one_at_a_time_in_order_with_the_first_read_ahead() {
    let first = row(Table::Frames, "a", 1);
    let rest = [row(Table::Frames, "a", 2), row(Table::Frames, "b", 0)];
    let mut stream = RowStream::new(vec![Table::Frames], Some(first), piped(&rest));
    assert_eq!(
        read(&mut stream, Table::Frames).unwrap(),
        ["a/1", "a/2", "b/0"]
    );
    let again = read(&mut stream, Table::Frames).unwrap_err();
    assert!(again.to_string().contains("once"), "{again}");
    let other = read(&mut stream, Table::Readings).unwrap_err();
    assert!(other.to_string().contains("no rows"), "{other}");
}

#[test]
fn a_later_table_waits_and_an_earlier_one_never_asked_for_is_skipped() {
    let rows = [
        row(Table::Frames, "a", 1),
        row(Table::Readings, "a", 1),
        row(Table::Readings, "a", 2),
    ];
    let tables = vec![Table::Frames, Table::Readings];
    let mut stream = RowStream::new(tables.clone(), None, piped(&rows));
    assert_eq!(read(&mut stream, Table::Readings).unwrap(), ["a/1", "a/2"]);
    let mut stream = RowStream::new(tables, None, piped(&rows));
    assert_eq!(read(&mut stream, Table::Frames).unwrap(), ["a/1"]);
    assert_eq!(read(&mut stream, Table::Readings).unwrap(), ["a/1", "a/2"]);
}

#[test]
fn a_row_of_a_table_not_sent_or_a_named_value_is_an_error() {
    let mut stream = RowStream::new(
        vec![Table::Frames],
        Some(row(Table::Readings, "a", 1)),
        piped(&[]),
    );
    assert!(read(&mut stream, Table::Frames).is_err());
    let named = (
        Address {
            table: Table::Frames,
            key: Key::Name("x".into()),
        },
        Vec::new(),
    );
    let mut stream = RowStream::new(vec![Table::Frames], Some(named), piped(&[]));
    let error = read(&mut stream, Table::Frames).unwrap_err();
    assert!(error.to_string().contains("named value"), "{error}");
}

#[test]
fn draining_reads_every_row_left_to_the_end() {
    let rows = [row(Table::Frames, "a", 1), row(Table::Frames, "a", 2)];
    let mut stream = RowStream::new(
        vec![Table::Frames],
        Some(row(Table::Frames, "a", 0)),
        piped(&rows),
    );
    stream.drain().unwrap();
    assert!(read(&mut stream, Table::Frames).unwrap().is_empty());
}
