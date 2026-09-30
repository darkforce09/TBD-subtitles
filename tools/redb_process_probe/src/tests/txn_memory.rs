use super::*;

const STATUS: &str = "\
Name:\tredb-process-pr
Umask:\t0022
State:\tR (running)
VmPeak:\t 2345678 kB
VmSize:\t 2345600 kB
VmHWM:\t 1843200 kB
VmRSS:\t 1048576 kB
RssAnon:\t 1000000 kB
Threads:\t1
";

fn scratch_dir(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "redb-process-probe-txn-memory-{label}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default()
    ))
}

fn tiny_options(dir: &Path, keep: bool) -> Options {
    Options {
        dir: dir.to_path_buf(),
        rows: 2000,
        value_bytes: 64,
        cache_mib: 16,
        occurrences: 40,
        keep,
    }
}

#[test]
fn the_status_parser_reads_rss_and_hwm_in_kb() {
    assert_eq!(
        parse_memory_status(STATUS),
        Some(MemoryStatus {
            rss_kb: 1_048_576,
            hwm_kb: 1_843_200,
        })
    );
}

#[test]
fn the_status_parser_needs_both_fields_as_kb_numbers() {
    assert_eq!(parse_memory_status("VmRSS:\t 12 kB\n"), None);
    assert_eq!(
        parse_memory_status("VmHWM:\t 12 kB\nVmRSS:\t lots kB\n"),
        None
    );
    assert_eq!(
        parse_memory_status("VmHWM:\t 12 kB\nVmRSSx:\t 12 kB\n"),
        None
    );
}

#[test]
fn the_live_status_parses() {
    let status = read_memory_status().expect("this process has a status");
    assert!(status.rss_kb > 0);
    assert!(status.hwm_kb >= status.rss_kb);
}

#[test]
fn checkpoints_fall_on_each_tenth_short_of_the_last_row() {
    assert_eq!(
        checkpoints(432_000),
        (1..10).map(|tenth| tenth * 43_200).collect::<Vec<_>>()
    );
    assert_eq!(checkpoints(5), vec![1, 2, 3, 4]);
    assert_eq!(checkpoints(1), Vec::<u64>::new());
}

#[test]
fn the_fill_differs_per_row_and_covers_a_ragged_tail() {
    let mut first = [0u8; 13];
    let mut second = [0u8; 13];
    fill_pseudo_random(&mut first, 0);
    fill_pseudo_random(&mut second, 1);
    assert_ne!(first, second);
    assert_ne!(first[8..], [0u8; 5]);
    let mut again = [0u8; 13];
    fill_pseudo_random(&mut again, 0);
    assert_eq!(first, again);
}

#[test]
fn a_tiny_run_counts_its_rows_and_removes_its_file() {
    let dir = scratch_dir("removed");
    std::fs::create_dir_all(&dir).expect("create the scratch folder");
    let mut samples = Vec::new();
    let result = run(&tiny_options(&dir, false), &mut |sample| {
        samples.push(sample.clone())
    });
    let file_left = dir.join(DATABASE_FILE).exists();
    let _ = std::fs::remove_dir_all(&dir);
    let summary = result.expect("the tiny run holds");

    assert_eq!(summary.rows, 2000);
    assert_eq!(summary.value_bytes, 64);
    assert!((summary.payload_mib - 2000.0 * 64.0 / MIB).abs() < 1e-9);
    assert!(summary.file_mib > 0.0, "the file existed after commit");
    assert!(summary.insert_seconds >= 0.0 && summary.commit_seconds >= 0.0);
    assert!(summary.rss_before_commit_mib > 0.0);
    assert!(summary.peak_hwm_mib >= summary.rss_before_commit_mib);
    assert!(!summary.kept);
    assert!(!file_left, "the database file is removed");

    let stages: Vec<&str> = samples.iter().map(|sample| sample.stage).collect();
    assert_eq!(samples.len(), 13, "{stages:?}");
    assert_eq!(stages.first(), Some(&"before transaction"));
    assert_eq!(
        &stages[10..],
        ["before commit", "after commit", "after drop"]
    );
    assert_eq!(samples[1].rows, 200);
    assert!(
        samples
            .iter()
            .all(|sample| sample.hwm_mib >= sample.rss_mib)
    );
    assert!(summary.render().iter().any(|line| line == "  rows: 2000"));
    assert!(samples[0].render().starts_with("rows 0 rss "));
}

#[test]
fn a_kept_run_leaves_its_file_with_every_row() {
    let dir = scratch_dir("kept");
    std::fs::create_dir_all(&dir).expect("create the scratch folder");
    let result = run(&tiny_options(&dir, true), &mut |_| {});
    let path = dir.join(DATABASE_FILE);
    let file_left = path.exists();
    let rows = file_left.then(|| {
        use redb::{ReadableDatabase, ReadableTableMetadata};
        let db = redb::Database::open(&path).expect("reopen the kept file");
        let transaction = db.begin_read().expect("begin a read");
        let table = transaction.open_table(FRAME_TABLE).expect("open frames");
        table.len().expect("count rows")
    });
    let _ = std::fs::remove_dir_all(&dir);
    let summary = result.expect("the tiny run holds");
    assert!(summary.kept);
    assert!(file_left, "the database file is kept");
    assert_eq!(rows, Some(2000));
}
