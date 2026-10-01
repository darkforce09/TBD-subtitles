use std::collections::BTreeMap;
use std::fs;

use clap::Parser;
use job_model::job::{StepMeasure, StepRecord};

use super::super::{Cli, Command};
use super::*;

fn dump_args(args: &[&str]) -> DumpArgs {
    let cli = Cli::try_parse_from(std::iter::once("tbd-subtitles").chain(args.iter().copied()))
        .expect("the arguments parse");
    match cli.command {
        Some(Command::Dump(args)) => args,
        other => panic!("expected dump, got {other:?}"),
    }
}

/// A fresh folder of its own under the temp folder.
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("tbd-dump-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("scratch folder");
    root
}

fn step_record() -> StepRecord {
    StepRecord {
        fingerprint: "ab12".into(),
        finished_ns: 7,
        measure: StepMeasure {
            wall_s: 3.5,
            notes: BTreeMap::new(),
            ..StepMeasure::default()
        },
    }
}

#[test]
fn dump_takes_a_job_a_table_and_an_optional_key() {
    let args = dump_args(&["dump", "episode.mkv", "meta", "layout"]);
    assert_eq!(args.job_or_video, PathBuf::from("episode.mkv"));
    assert_eq!(args.table, Table::Meta);
    assert_eq!(args.key.as_deref(), Some("layout"));
    let args = dump_args(&["dump", "job-1234", "step_records", "--work-root", "/w"]);
    assert_eq!(args.table, Table::StepRecords);
    assert_eq!(args.key, None);
    assert_eq!(args.work_root, Some(PathBuf::from("/w")));
    let parse = |args: &[&str]| {
        Cli::try_parse_from(std::iter::once("tbd-subtitles").chain(args.iter().copied()))
    };
    assert!(parse(&["dump", "episode.mkv"]).is_err());
    let error = parse(&["dump", "episode.mkv", "tables"]).expect_err("no such table");
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn a_key_takes_the_form_of_its_table() {
    assert_eq!(
        parse_key(Table::Outputs, "cues/dropped_sounds"),
        Ok(Key::Name("cues/dropped_sounds".into()))
    );
    assert_eq!(
        parse_key(Table::Frames, "sign/3/120"),
        Ok(Key::Frame {
            occurrence: "sign/3".into(),
            frame: 120
        })
    );
    assert!(parse_key(Table::Readings, "sign").is_err());
    assert!(parse_key(Table::Readings, "/12").is_err());
    assert!(parse_key(Table::Frames, "sign/x").is_err());
    assert!(parse_key(Table::Meta, "").is_err());
}

#[test]
fn a_job_is_found_by_its_folder_its_name_or_its_video() {
    let root = scratch("resolve");
    let work_root = root.join("work");
    let job = work_root.join("episode-01-cafe");
    fs::create_dir_all(&job).expect("job folder");
    fs::write(job.join("job.redb"), b"").expect("database");

    assert_eq!(resolve(&job, &work_root).expect("folder").root(), job);
    assert_eq!(
        resolve(Path::new("episode-01-cafe"), &work_root)
            .expect("name")
            .root(),
        job
    );

    let video = root.join("Episode 01.mkv");
    fs::write(&video, b"video").expect("video");
    let canonical = fs::canonicalize(&video).expect("canonical");
    assert_eq!(
        resolve(&video, &work_root).expect("video").root(),
        work_root.join(job_id(&canonical))
    );

    let error = resolve(&root.join("nothing.mkv"), &work_root).expect_err("missing");
    assert!(error.to_string().contains("no job folder"), "{error}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_store_dumps_one_row_pretty_or_a_table_as_json_lines() {
    let root = scratch("print");
    let store = JobStore::open(&WorkDir::new(root.join("job"))).expect("store");
    let mut write = store.write().expect("write");
    write
        .put(Table::StepRecords, &Key::Name("vad".into()), &step_record())
        .expect("put");
    write
        .put(
            Table::Outputs,
            &Key::Name("separation".into()),
            &step_record(),
        )
        .expect("put");
    write.commit().expect("commit");
    let read = store.read().expect("read");

    let mut out = Vec::new();
    assert!(
        print(
            &read,
            Table::Meta,
            Some(&Key::Name("layout".into())),
            &mut out
        )
        .unwrap()
    );
    let text = String::from_utf8(out).expect("UTF-8");
    assert!(text.contains("\n  \"versions\": {"), "{text}");
    let layout: Value = serde_json::from_str(&text).expect("JSON");
    assert_eq!(layout["versions"]["outputs"], 3);

    let mut out = Vec::new();
    assert!(print(&read, Table::StepRecords, None, &mut out).unwrap());
    let lines: Vec<Value> = String::from_utf8(out)
        .expect("UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("one JSON object per line"))
        .collect();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0]["key"], "vad");
    assert_eq!(lines[0]["value"]["fingerprint"], "ab12");
    assert_eq!(lines[0]["value"]["measure"]["wall_s"], 3.5);

    let mut out = Vec::new();
    assert!(print(&read, Table::Outputs, None, &mut out).unwrap());
    let line: Value = serde_json::from_slice(&out).expect("JSON");
    assert_eq!(line["key"], "separation");
    assert_eq!(line["value"], Value::Null);
    assert!(line["bytes"].as_u64().is_some_and(|bytes| bytes > 0));

    let mut out = Vec::new();
    let missing = print(
        &read,
        Table::Meta,
        Some(&Key::Name("job_record".into())),
        &mut out,
    );
    assert!(!missing.unwrap());
    assert!(out.is_empty());
    drop(read);
    drop(store);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn every_row_the_pipeline_keeps_prints_as_its_record() {
    use job_model::StepName;
    use job_model::job::{JobRecord, JobSettings};
    use job_model::outputs::FixRecord;
    use pipeline::work_dir::store::keys;
    let root = scratch("every-row");
    let store = JobStore::open(&WorkDir::new(root.join("job"))).expect("store");
    store
        .put_job_record(&JobRecord {
            video: "/videos/episode.mkv".into(),
            video_size: 1,
            video_modified_s: 0,
            settings: JobSettings::with_glossary(Vec::new()),
            models_dir: None,
            corrections: None,
        })
        .expect("record");
    store
        .put_step_record(StepName::TextTypeset, &step_record())
        .expect("step");
    store
        .put_output(
            StepName::TextTypeset,
            Some(keys::TYPESET_ASS),
            &"Dialogue: 0,0:00:01.00".to_string(),
        )
        .expect("ass");
    store
        .put_output(
            StepName::Cues,
            Some(keys::DROPPED_SOUNDS),
            &vec!["[THUD]".to_string()],
        )
        .expect("dropped");
    pipeline::work_dir::put_fix_record(&store, &FixRecord::default()).expect("fix");
    pipeline::work_dir::update_text_corrections(&store, |c| c.retry.push("t1".into()))
        .expect("text corrections");
    let read = store.read().expect("read");
    for (table, expected) in [
        (Table::Meta, vec!["job_record", "layout"]),
        (Table::StepRecords, vec!["text_typeset"]),
        (
            Table::Outputs,
            vec!["cues/dropped_sounds", "text_typeset/ass"],
        ),
        (Table::Corrections, vec!["fix", "text"]),
    ] {
        let mut out = Vec::new();
        assert!(print(&read, table, None, &mut out).unwrap());
        let lines: Vec<Value> = String::from_utf8(out)
            .expect("UTF-8")
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON"))
            .collect();
        let keys: Vec<&str> = lines
            .iter()
            .map(|line| line["key"].as_str().expect("a key"))
            .collect();
        assert_eq!(keys, expected, "{table}");
        for line in &lines {
            assert!(!line["value"].is_null(), "{table} {line}");
        }
    }
    let mut out = Vec::new();
    let key = Key::Name("text_typeset/ass".into());
    assert!(print(&read, Table::Outputs, Some(&key), &mut out).unwrap());
    assert_eq!(
        serde_json::from_slice::<Value>(&out).unwrap(),
        "Dialogue: 0,0:00:01.00"
    );
    drop(read);
    drop(store);
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn frame_rows_print_as_frame_records_and_readings_whole_or_one_by_key() {
    use job_model::onscreen::{FrameRecord, RleRun, VerifyReading};
    let root = scratch("frames");
    let store = JobStore::open(&WorkDir::new(root.join("job"))).expect("store");
    let mut write = store.write().expect("write");
    for frame in [3u64, 4] {
        let key = Key::Frame {
            occurrence: "t1".into(),
            frame,
        };
        let row = FrameRecord {
            shift: [frame as f64, 0.0],
            scale: 1.0,
            mask: vec![RleRun {
                row: 0,
                start: 1,
                len: 2,
            }],
            ..FrameRecord::default()
        };
        write.put(Table::Frames, &key, &row).expect("frame");
        let reading = VerifyReading {
            frame,
            english_read: "SEA".into(),
            ..VerifyReading::default()
        };
        write.put(Table::Readings, &key, &reading).expect("reading");
    }
    write.commit().expect("commit");
    let read = store.read().expect("read");
    let mut out = Vec::new();
    assert!(print(&read, Table::Frames, None, &mut out).unwrap());
    let lines: Vec<Value> = String::from_utf8(out)
        .expect("UTF-8")
        .lines()
        .map(|line| serde_json::from_str(line).expect("JSON"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["key"], "t1/3");
    assert_eq!(lines[1]["value"]["shift"][0], 4.0);
    assert_eq!(lines[1]["value"]["mask"][0]["len"], 2);
    let key = parse_key(Table::Readings, "t1/4").expect("a key");
    let mut out = Vec::new();
    assert!(print(&read, Table::Readings, Some(&key), &mut out).unwrap());
    let one: Value = serde_json::from_slice(&out).expect("JSON");
    assert_eq!(one["english_read"], "SEA");
    assert_eq!(one["frame"], 4);
    drop(read);
    drop(store);
    let _ = fs::remove_dir_all(&root);
}
