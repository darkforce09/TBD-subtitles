use std::fs;
use std::sync::Arc;

use job_model::onscreen::{Point, Quad};
use job_model::store::TableLayouts;
use redb::{Database, DatabaseError, TableHandle};
use worker_channel::address::{Key, Table};

use super::*;
use crate::error::ErrorKind;

fn scratch(name: &str) -> WorkDir {
    let dir = std::env::temp_dir().join(format!("tbd-store-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    WorkDir::new(dir)
}

fn table_names(database: &Database) -> Vec<String> {
    let transaction = database.begin_write().expect("write");
    let mut names: Vec<String> = transaction
        .list_tables()
        .expect("list")
        .map(|table| table.name().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn an_open_creates_the_six_tables_the_layout_and_the_lock() {
    let work = scratch("create");
    let store = JobStore::open(&work).expect("open");
    assert_eq!(store.path(), work.database());
    assert_eq!(
        fs::read_to_string(work.lock()).expect("lock"),
        std::process::id().to_string()
    );
    let layouts: TableLayouts = store
        .read()
        .expect("read")
        .get(Table::Meta, &Key::Name(tables::LAYOUT_KEY.into()))
        .expect("get")
        .expect("a layout");
    assert_eq!(layouts.versions.len(), 6);
    for (table, version) in LAYOUT_VERSIONS {
        assert_eq!(layouts.versions.get(table.name()), Some(&version));
    }
    drop(store);

    let database = Database::create(work.database()).expect("reopen");
    let mut expected: Vec<String> = Table::ALL.iter().map(|t| t.name().to_string()).collect();
    expected.sort();
    assert_eq!(table_names(&database), expected);
    drop(database);
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn every_caller_in_a_process_shares_one_handle_and_the_last_one_closes_it() {
    let work = scratch("shared");
    let first = JobStore::open(&work).expect("open");
    let second = JobStore::open(&work).expect("open again");
    assert!(Arc::ptr_eq(&first, &second));
    let from_thread = {
        let work = work.clone();
        std::thread::spawn(move || JobStore::open(&work).expect("open on a thread"))
            .join()
            .expect("thread")
    };
    assert!(Arc::ptr_eq(&first, &from_thread));
    drop((first, second));
    assert!(work.lock().exists(), "one handle is still held");
    drop(from_thread);
    assert!(!work.lock().exists());

    let again = JobStore::open(&work).expect("open after close");
    assert!(work.lock().exists());
    drop(again);
    assert!(!work.lock().exists());
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_second_handle_is_refused_and_a_foreign_owner_is_busy() {
    let work = scratch("busy");
    let store = JobStore::open(&work).expect("open");
    assert!(matches!(
        Database::create(work.database()),
        Err(DatabaseError::DatabaseAlreadyOpen)
    ));
    drop(store);
    assert!(!work.lock().exists());

    // Another process's hold: the file open outside the store, and its pid in `job.lock`.
    let foreign = Database::create(work.database()).expect("a foreign hold");
    let parent = std::os::unix::process::parent_id();
    fs::write(work.lock(), parent.to_string()).expect("lock");
    let refused = JobStore::open(&work).expect_err("busy");
    assert_eq!(
        refused.kind,
        ErrorKind::Busy {
            owner: Some(parent)
        }
    );
    assert_eq!(refused.busy_owner(), Some(Some(parent)));
    assert_eq!(
        refused.message,
        format!("process {parent} is already running it")
    );
    assert_eq!(
        fs::read_to_string(work.lock()).expect("the owner's lock stays"),
        parent.to_string()
    );

    fs::remove_file(work.lock()).expect("remove");
    let unknown = JobStore::open_existing(&work).expect_err("busy");
    assert_eq!(unknown.busy_owner(), Some(None));
    assert_eq!(unknown.message, "another process is already running it");
    assert!(!work.lock().exists());

    drop(foreign);
    let store = JobStore::open(&work).expect("free again");
    drop(store);
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_changed_layout_version_drops_that_table_and_keeps_the_others() {
    let work = scratch("layout");
    fs::create_dir_all(work.root()).expect("folder");
    let database = Database::create(work.database()).expect("create");
    apply_layouts(&database, &LAYOUT_VERSIONS, &work.database()).expect("layout");
    let quad = Quad([Point { x: 1.0, y: 2.0 }; 4]);
    let frame = Key::Frame {
        occurrence: "T0001".into(),
        frame: 3,
    };
    let output = Key::Name("vad".into());
    let path = work.database();
    let mut write = StoreWrite::new(database.begin_write().expect("write"), path.clone());
    write.put(Table::Frames, &frame, &quad).expect("frame");
    write.put(Table::Outputs, &output, &quad).expect("output");
    write.commit().expect("commit");

    let mut bumped = LAYOUT_VERSIONS;
    for (table, version) in &mut bumped {
        if *table == Table::Frames {
            *version += 1;
        }
    }
    apply_layouts(&database, &bumped, &path).expect("bumped layout");
    let read = StoreRead::new(database.begin_read().expect("read"), path.clone());
    assert!(read.raw(Table::Frames, &frame).expect("frames").is_none());
    assert!(
        read.raw(Table::Outputs, &output)
            .expect("outputs")
            .is_some()
    );
    let layouts: TableLayouts = read
        .get(Table::Meta, &Key::Name(tables::LAYOUT_KEY.into()))
        .expect("get")
        .expect("a layout");
    assert_eq!(layouts.versions.get("frames"), Some(&2));
    drop(read);

    // The same versions again drop nothing.
    apply_layouts(&database, &bumped, &path).expect("same layout");
    let read = StoreRead::new(database.begin_read().expect("read"), path);
    assert!(
        read.raw(Table::Outputs, &output)
            .expect("outputs")
            .is_some()
    );
    drop((read, database));
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn a_table_without_a_stored_version_is_dropped() {
    let work = scratch("no-layout");
    fs::create_dir_all(work.root()).expect("folder");
    let database = Database::create(work.database()).expect("create");
    {
        let transaction = database.begin_write().expect("write");
        {
            let mut outputs = transaction.open_table(tables::OUTPUTS).expect("outputs");
            outputs.insert("vad", [1u8, 2, 3].as_slice()).expect("row");
        }
        transaction.commit().expect("commit");
    }
    apply_layouts(&database, &LAYOUT_VERSIONS, &work.database()).expect("layout");
    let read = StoreRead::new(database.begin_read().expect("read"), work.database());
    assert!(
        read.raw(Table::Outputs, &Key::Name("vad".into()))
            .expect("outputs")
            .is_none()
    );
    drop((read, database));
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn opening_an_existing_store_never_creates_one() {
    let work = scratch("existing");
    assert!(JobStore::open_existing(&work).is_err());
    assert!(!work.root().exists());

    fs::create_dir_all(work.root()).expect("folder");
    let failed = JobStore::open_existing(&work).expect_err("no database");
    assert_eq!(failed.kind, ErrorKind::Failed);
    assert!(failed.context.contains("job.redb"), "{failed}");
    assert!(!work.database().exists());
    assert!(!work.lock().exists());

    drop(JobStore::open(&work).expect("create"));
    let existing = JobStore::open_existing(&work).expect("open existing");
    assert!(work.lock().exists());
    drop(existing);
    let _ = fs::remove_dir_all(work.root());
}
