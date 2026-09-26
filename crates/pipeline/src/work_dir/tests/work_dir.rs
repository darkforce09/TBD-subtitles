use super::*;

#[test]
fn a_job_id_is_a_slug_of_the_name_and_a_hash_of_the_path() {
    let id = job_id(Path::new("/media/one_pace/[Muhn Pace] Dressrosa 11.mp4"));
    assert!(id.starts_with("muhn-pace-dressrosa-11-"), "{id}");
    assert_eq!(id.len(), "muhn-pace-dressrosa-11-".len() + 8);
    assert_ne!(
        id,
        job_id(Path::new("/elsewhere/[Muhn Pace] Dressrosa 11.mp4"))
    );
    assert_eq!(
        id,
        job_id(Path::new("/media/one_pace/[Muhn Pace] Dressrosa 11.mp4"))
    );
}

#[test]
fn json_round_trips_and_leaves_no_part_file() {
    let dir = std::env::temp_dir().join(format!("tbd-work-dir-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let work = WorkDir::new(&dir);
    write_json(&work.asr("parakeet"), &vec![1, 2, 3]).expect("write");
    let back: Vec<i32> = read_json(&work.asr("parakeet")).expect("read");
    assert_eq!(back, vec![1, 2, 3]);
    assert!(!dir.join("asr/parakeet.json.part").exists());
    assert!(read_json::<Vec<i32>>(&work.cues()).is_err());
    let _ = fs::remove_dir_all(&dir);
}
