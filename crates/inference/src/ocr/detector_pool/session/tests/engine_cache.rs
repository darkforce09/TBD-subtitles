use super::*;

fn identity() -> EngineIdentity {
    EngineIdentity {
        gpu_name: "NVIDIA GeForce RTX 3070".to_owned(),
        driver: "580.95.05".to_owned(),
        tensorrt_version: "10.14.1.48".to_owned(),
    }
}

const SHAPE: [usize; 4] = [4, 3, 1088, 1920];

#[test]
fn every_field_changes_the_key() {
    let base = cache_key(&identity(), "aa", SHAPE, true);
    assert_eq!(base.len(), 32);
    assert!(base.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(base, cache_key(&identity(), "aa", SHAPE, true));
    let mut changed = Vec::new();
    for field in 0..3 {
        let mut other = identity();
        match field {
            0 => other.gpu_name.push('X'),
            1 => other.driver.push('1'),
            _ => other.tensorrt_version.push('1'),
        }
        changed.push(cache_key(&other, "aa", SHAPE, true));
    }
    changed.push(cache_key(&identity(), "ab", SHAPE, true));
    changed.push(cache_key(&identity(), "aa", [1, 3, 1088, 1920], true));
    changed.push(cache_key(&identity(), "aa", SHAPE, false));
    for key in &changed {
        assert_ne!(*key, base);
    }
    let mut unique = changed.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), changed.len());
}

#[test]
fn fields_cannot_run_into_each_other() {
    let mut joined = identity();
    joined.gpu_name = "RTX".to_owned();
    joined.driver = "3070580".to_owned();
    let mut split = identity();
    split.gpu_name = "RTX3070".to_owned();
    split.driver = "580".to_owned();
    assert_ne!(
        cache_key(&joined, "aa", SHAPE, true),
        cache_key(&split, "aa", SHAPE, true)
    );
}

#[test]
fn an_unknown_identity_field_is_refused() {
    assert!(check_identity(&identity()).is_ok());
    let mut missing = identity();
    missing.driver = " ".to_owned();
    let error = check_identity(&missing).unwrap_err().to_string();
    assert!(error.contains("driver"), "{error}");
    assert!(check_identity(&EngineIdentity::default()).is_err());
}

#[test]
fn profiles_name_the_input_and_its_one_shape() {
    assert_eq!(profile_shapes("x", SHAPE), "x:4x3x1088x1920");
    assert_eq!(profile_shapes("x", [1, 3, 1088, 1920]), "x:1x3x1088x1920");
}

#[test]
fn the_workspace_keeps_the_sessions_within_the_cap() {
    // Two screening sessions with 2,304 MiB arenas under 6.5 GB.
    assert_eq!(workspace_mib(6_656, 2, 2_304), 768);
    assert!(2 * (2_304 + workspace_mib(6_656, 2, 2_304)) + 512 <= 6_656);
    assert_eq!(workspace_mib(6_656, 2, 1_536), 1_536);
    assert_eq!(workspace_mib(4_000, 2, 2_304), 256);
}

#[test]
fn a_folder_holds_an_engine_once_an_engine_file_is_in_it() {
    let folder = std::env::temp_dir().join(format!("tensorrt-cache-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    assert!(!holds_engine(&folder));
    std::fs::write(folder.join("model.timing"), b"").unwrap();
    assert!(!holds_engine(&folder));
    std::fs::write(folder.join("model_sm86.engine"), b"").unwrap();
    assert!(holds_engine(&folder));
    std::fs::remove_dir_all(&folder).unwrap();
    assert!(!holds_engine(&folder));
}

#[test]
fn sha256_is_lowercase_hex() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
