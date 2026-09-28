use job_model::outputs::{Chosen, Correction};

use super::*;

fn scratch(name: &str) -> WorkDir {
    let dir = std::env::temp_dir().join(format!("tbd-corrections-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    WorkDir::new(dir)
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
fn a_change_is_written_and_the_last_one_taken_out_removes_the_file() {
    let work = scratch("write");
    let (after, added) = update_corrections(&work, |c| {
        c.set(typed("U0001"));
        c.lines.len()
    })
    .unwrap();
    assert_eq!(added, 1);
    assert_eq!(read_corrections(&work).unwrap(), after);
    assert!(corrections_digest(&work).is_some());
    update_corrections(&work, |c| c.remove("U0001")).unwrap();
    assert!(!work.review().exists());
    assert!(corrections_digest(&work).is_none());
    assert_eq!(read_corrections(&work).unwrap(), Corrections::default());
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn nothing_changed_writes_nothing() {
    let work = scratch("unchanged");
    update_corrections(&work, |c| c.set(typed("U0001"))).unwrap();
    let written = fs::metadata(work.review()).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    update_corrections(&work, |_| ()).unwrap();
    let again = fs::metadata(work.review()).unwrap().modified().unwrap();
    assert_eq!(written, again);
    let _ = fs::remove_dir_all(work.root());
}

#[test]
fn two_writers_at_once_lose_no_change() {
    let work = scratch("writers");
    std::thread::scope(|scope| {
        for writer in 0..4 {
            let work = &work;
            scope.spawn(move || {
                for line in 0..10 {
                    update_corrections(work, |c| c.set(typed(&format!("U{writer}{line:03}"))))
                        .unwrap();
                }
            });
        }
    });
    assert_eq!(read_corrections(&work).unwrap().lines.len(), 40);
    let _ = fs::remove_dir_all(work.root());
}
