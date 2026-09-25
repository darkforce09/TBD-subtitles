use super::*;

struct TmpDir(PathBuf);
impl TmpDir {
    fn new(name: &str) -> TmpDir {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "verification-core-scan-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TmpDir(p)
    }
    fn file(&self, rel: &str, body: &str) -> PathBuf {
        let p = self.0.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, body).unwrap();
        p
    }
}
impl Drop for TmpDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_missing_root_is_did_not_run_not_zero_hits() {
    // THE DEFECT. A search whose error is silenced reads a renamed directory as "clean".
    let got = walk_files(&[Path::new("/nonexistent/verification-core/scan")], |_| {
        true
    });
    assert!(matches!(got, Err(NotRun::TargetMissing(_))));
}

#[test]
fn walks_recursively_and_deterministically() {
    let d = TmpDir::new("walk");
    d.file("a.rs", "");
    d.file("sub/b.rs", "");
    d.file("sub/deep/c.rs", "");
    let files = walk_files(&[&d.0], |_| true).unwrap();
    assert_eq!(files.len(), 3);
    let mut sorted = files.clone();
    sorted.sort();
    assert_eq!(files, sorted, "order must not depend on readdir");
}

#[test]
fn extension_filter_applies() {
    let d = TmpDir::new("ext");
    d.file("keep.rs", "");
    d.file("drop.txt", "");
    let files = walk_files(&[&d.0], with_extension(&["rs"])).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].ends_with("keep.rs"));
}

#[test]
fn a_file_root_is_accepted_directly() {
    let d = TmpDir::new("fileroot");
    let f = d.file("solo.rs", "");
    assert_eq!(walk_files(&[&f], |_| true).unwrap(), vec![f.clone()]);
}

#[test]
fn matching_lines_reports_one_based_line_numbers() {
    let d = TmpDir::new("grep");
    let f = d.file("x.rs", "first\nSELECT * FROM users\nthird\n");
    let hits = matching_lines(
        &Pattern::regex("SELECT \\* FROM").unwrap(),
        std::slice::from_ref(&f),
    )
    .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].line_no, 2, "line numbers are 1-based");
    assert_eq!(hits[0].line, "SELECT * FROM users");
    assert!(hits[0].rendered().ends_with(":2:SELECT * FROM users"));
}

#[test]
fn matching_lines_finds_every_occurrence() {
    let d = TmpDir::new("multi");
    let f = d.file("y.rs", "hit\nmiss\nhit\n");
    let hits = matching_lines(&Pattern::literal("hit"), &[f]).unwrap();
    assert_eq!(
        hits.iter().map(|h| h.line_no).collect::<Vec<_>>(),
        vec![1, 3]
    );
}

#[test]
fn matching_lines_on_a_missing_file_is_did_not_run() {
    let got = matching_lines(
        &Pattern::literal("x"),
        &[PathBuf::from("/nonexistent/tbd/z.rs")],
    );
    assert!(matches!(got, Err(NotRun::Unreadable { .. })));
}

#[test]
fn non_utf8_bytes_do_not_abort_the_scan() {
    // A stray latin-1 byte in a source file must not make the gate unable to run.
    let d = TmpDir::new("binary");
    let p = d.0.join("odd.rs");
    std::fs::write(&p, [b'h', b'i', 0xff, b'\n', b'x', b'\n']).unwrap();
    let hits = matching_lines(&Pattern::literal("x"), &[p]).unwrap();
    assert_eq!(hits.len(), 1);
}
