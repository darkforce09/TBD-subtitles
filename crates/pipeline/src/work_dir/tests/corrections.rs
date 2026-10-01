use std::sync::Arc;

use job_model::outputs::{Chosen, Correction};

use super::*;
use crate::work_dir::WorkDir;

/// A job database in a folder of its own, removed when the test ends.
struct Scratch {
    store: Option<Arc<JobStore>>,
    dir: std::path::PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("tbd-corrections-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = JobStore::open(&WorkDir::new(dir.clone())).expect("the store opens");
        Scratch {
            store: Some(store),
            dir,
        }
    }

    fn store(&self) -> &JobStore {
        self.store.as_ref().expect("open")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(self.store.take());
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn typed(id: &str) -> Correction {
    Correction {
        id: id.into(),
        text: format!("Line {id}."),
        flags: Vec::new(),
        chosen: Chosen::Typed,
    }
}

#[test]
fn a_change_is_stored_and_the_last_one_taken_out_removes_the_row() {
    let scratch = Scratch::new("write");
    let store = scratch.store();
    let (after, added) = update_corrections(store, |c| {
        c.set(typed("U0001"));
        c.lines.len()
    })
    .unwrap();
    assert_eq!(added, 1);
    assert_eq!(read_corrections(store).unwrap(), after);
    assert!(corrections_digest(store).unwrap().is_some());
    update_corrections(store, |c| c.remove("U0001")).unwrap();
    let row = store
        .read()
        .unwrap()
        .raw(Table::Corrections, &keys::named(keys::LINE_CORRECTIONS))
        .unwrap();
    assert!(row.is_none());
    assert!(corrections_digest(store).unwrap().is_none());
    assert_eq!(read_corrections(store).unwrap(), Corrections::default());
    assert!(!scratch.dir.join("review.json").exists());
    assert!(!scratch.dir.join("review.json.lock").exists());
}

#[test]
fn nothing_changed_writes_nothing() {
    let scratch = Scratch::new("unchanged");
    let store = scratch.store();
    update_corrections(store, |c| c.set(typed("U0001"))).unwrap();
    let digest = corrections_digest(store).unwrap();
    update_corrections(store, |_| ()).unwrap();
    update_corrections(store, |c| c.set(typed("U0001"))).unwrap();
    assert_eq!(corrections_digest(store).unwrap(), digest);
}

#[test]
fn writers_at_once_lose_no_change() {
    let scratch = Scratch::new("writers");
    let store = scratch.store();
    std::thread::scope(|scope| {
        for writer in 0..4 {
            scope.spawn(move || {
                for line in 0..10 {
                    update_corrections(store, |c| c.set(typed(&format!("U{writer}{line:03}"))))
                        .unwrap();
                }
            });
        }
    });
    assert_eq!(read_corrections(store).unwrap().lines.len(), 40);
}

#[test]
fn text_corrections_are_their_own_row_with_their_own_digest() {
    let scratch = Scratch::new("text");
    let store = scratch.store();
    assert_eq!(
        read_text_corrections(store).unwrap(),
        TextCorrections::default()
    );
    let (after, ()) = update_text_corrections(store, |c| {
        c.retry.push("t0001".into());
    })
    .unwrap();
    assert_eq!(read_text_corrections(store).unwrap(), after);
    assert!(corrections_digest(store).unwrap().is_none());
    let read = store.read().unwrap();
    assert!(digest_in(&read, keys::TEXT_CORRECTIONS).unwrap().is_some());
    drop(read);
    update_text_corrections(store, |c| *c = TextCorrections::default()).unwrap();
    let read = store.read().unwrap();
    assert!(digest_in(&read, keys::TEXT_CORRECTIONS).unwrap().is_none());
}
