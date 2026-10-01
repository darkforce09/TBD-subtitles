use std::collections::BTreeMap;
use std::fs;
use std::io;

use job_model::job::{StepMeasure, StepRecord};
use job_model::onscreen::{Point, Quad};
use worker_channel::address::{Key, Table};

use crate::work_dir::{JobStore, WorkDir};

fn scratch(name: &str) -> WorkDir {
    let dir = std::env::temp_dir().join(format!("tbd-store-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    WorkDir::new(dir)
}

fn step_record() -> StepRecord {
    StepRecord {
        fingerprint: "ab12".into(),
        finished_ns: 1_700_000_000_000_000_000,
        measure: StepMeasure {
            wall_s: 12.5,
            load_s: Some(1.5),
            process_s: Some(11.0),
            peak_ram_mib: Some(512.0),
            peak_child_ram_mib: None,
            peak_vram_mib: Some(2048.0),
            job_ram_mib: Some(3072.0),
            cpu_cores_mean: Some(1.5),
            notes: BTreeMap::from([("chunks".to_string(), "7".to_string())]),
            ..StepMeasure::default()
        },
    }
}

fn quad() -> Quad {
    Quad([
        Point { x: 10.0, y: 20.0 },
        Point { x: 110.0, y: 20.0 },
        Point { x: 110.0, y: 60.0 },
        Point { x: 10.0, y: 60.0 },
    ])
}

fn frame(occurrence: &str, frame: u64) -> Key {
    Key::Frame {
        occurrence: occurrence.into(),
        frame,
    }
}

#[test]
fn a_step_record_comes_back_by_get_and_by_view() {
    let work = scratch("step-record");
    let store = JobStore::open(&work).expect("open");
    let key = Key::Name("vad".into());
    let mut write = store.write().expect("write");
    write
        .put(Table::StepRecords, &key, &step_record())
        .expect("put");
    write.commit().expect("commit");

    let read = store.read().expect("read");
    let back: Option<StepRecord> = read.get(Table::StepRecords, &key).expect("get");
    assert_eq!(back, Some(step_record()));
    let fingerprint = read
        .view::<StepRecord, _>(Table::StepRecords, &key, |archived| {
            archived.fingerprint.as_str().to_string()
        })
        .expect("view");
    assert_eq!(fingerprint.as_deref(), Some("ab12"));
    let missing: Option<StepRecord> = read
        .get(Table::StepRecords, &Key::Name("qc".into()))
        .expect("get a missing row");
    assert_eq!(missing, None);
    drop((read, store));
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_frame_row_comes_back_and_lists_in_frame_order() {
    let work = scratch("frames");
    let store = JobStore::open(&work).expect("open");
    let mut write = store.write().expect("write");
    for number in [42, 7] {
        write
            .put(Table::Frames, &frame("T0003", number), &quad())
            .expect("put");
    }
    write.commit().expect("commit");

    let read = store.read().expect("read");
    let back: Option<Quad> = read.get(Table::Frames, &frame("T0003", 42)).expect("get");
    assert_eq!(back, Some(quad()));
    let left = read
        .view::<Quad, _>(Table::Frames, &frame("T0003", 7), |archived| {
            archived.0[0].x.to_native()
        })
        .expect("view");
    assert_eq!(left, Some(10.0));
    assert_eq!(
        read.keys(Table::Frames).expect("keys"),
        vec![frame("T0003", 7), frame("T0003", 42)]
    );
    assert!(read.keys(Table::Readings).expect("keys").is_empty());
    drop((read, store));
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_key_of_the_wrong_kind_is_an_error() {
    let work = scratch("key-kind");
    let store = JobStore::open(&work).expect("open");
    let mut write = store.write().expect("write");
    assert!(
        write
            .put(Table::Outputs, &frame("T0001", 1), &quad())
            .is_err()
    );
    assert!(
        write
            .put(Table::Readings, &Key::Name("vad".into()), &quad())
            .is_err()
    );
    drop(write);
    let read = store.read().expect("read");
    assert!(read.raw(Table::Frames, &Key::Name("vad".into())).is_err());
    assert!(read.get::<Quad>(Table::Meta, &frame("T0001", 1)).is_err());
    drop((read, store));
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_reserved_row_is_filled_in_place_and_a_failed_fill_leaves_none() {
    let work = scratch("reserve");
    let store = JobStore::open(&work).expect("open");
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&step_record()).expect("archive");
    let mut source = io::Cursor::new(bytes.to_vec());
    let mut write = store.write().expect("write");
    write
        .reserve(
            Table::StepRecords,
            &Key::Name("cues".into()),
            bytes.len(),
            |slot| io::Read::read_exact(&mut source, slot),
        )
        .expect("reserve");
    let failed = write.reserve(Table::StepRecords, &Key::Name("qc".into()), 16, |_| {
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "the pipe closed",
        ))
    });
    assert!(failed.is_err());
    assert!(
        !write
            .remove(Table::StepRecords, &Key::Name("qc".into()))
            .expect("remove")
    );
    write.commit().expect("commit");

    let read = store.read().expect("read");
    let back: Option<StepRecord> = read
        .get(Table::StepRecords, &Key::Name("cues".into()))
        .expect("get");
    assert_eq!(back, Some(step_record()));
    assert_eq!(
        read.keys(Table::StepRecords).expect("keys"),
        vec![Key::Name("cues".into())]
    );
    drop((read, store));
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_removed_row_is_gone_and_an_uncommitted_write_changes_nothing() {
    let work = scratch("remove");
    let store = JobStore::open(&work).expect("open");
    let key = Key::Name("probe_decode".into());
    let mut write = store.write().expect("write");
    write
        .put(Table::Outputs, &key, &step_record())
        .expect("put");
    write.commit().expect("commit");

    let mut write = store.write().expect("write");
    assert!(write.remove(Table::Outputs, &key).expect("remove"));
    drop(write);
    assert!(
        store
            .read()
            .expect("read")
            .raw(Table::Outputs, &key)
            .expect("raw")
            .is_some()
    );

    let mut write = store.write().expect("write");
    assert!(write.remove(Table::Outputs, &key).expect("remove"));
    write.commit().expect("commit");
    assert!(
        store
            .read()
            .expect("read")
            .raw(Table::Outputs, &key)
            .expect("raw")
            .is_none()
    );
    drop(store);
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_per_frame_table_is_walked_in_key_order_whole_or_for_one_occurrence() {
    use job_model::onscreen::VerifyReading;

    let work = scratch("rows");
    let store = JobStore::open(&work).expect("open");
    let mut write = store.write().expect("write");
    for (occurrence, frame) in [("b", 1u64), ("a", 30), ("a", 4), ("ab", 0)] {
        let key = Key::Frame {
            occurrence: occurrence.into(),
            frame,
        };
        let reading = VerifyReading {
            frame,
            english_read: format!("{occurrence}{frame}"),
            ..VerifyReading::default()
        };
        write.put(Table::Readings, &key, &reading).expect("put");
    }
    write.commit().expect("commit");
    let read = store.read().expect("read");
    let mut all = Vec::new();
    read.rows_as::<VerifyReading>(Table::Readings, None, |id, frame, reading| {
        assert_eq!(reading.english_read, format!("{id}{frame}"));
        all.push((id.to_string(), frame));
        Ok(())
    })
    .expect("walk");
    let pairs = |list: &[(&str, u64)]| -> Vec<(String, u64)> {
        list.iter().map(|(id, f)| (id.to_string(), *f)).collect()
    };
    assert_eq!(all, pairs(&[("a", 4), ("a", 30), ("ab", 0), ("b", 1)]));
    let mut one = Vec::new();
    read.rows(Table::Readings, Some("a"), |id, frame, _| {
        one.push((id.to_string(), frame));
        Ok(())
    })
    .expect("walk one");
    assert_eq!(one, pairs(&[("a", 4), ("a", 30)]));
    assert!(read.rows(Table::Outputs, None, |_, _, _| Ok(())).is_err());
    drop(read);
    drop(store);
    let _ = fs::remove_dir_all(work.root());
}
