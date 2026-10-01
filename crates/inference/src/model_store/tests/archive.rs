use std::fs;
use std::path::PathBuf;

use super::*;

fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("archive-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(gzip);
    for (path, body) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        tar.append_data(&mut header, path, *body).unwrap();
    }
    tar.into_inner().unwrap().finish().unwrap()
}

#[test]
fn a_tar_gz_is_gunzipped_and_its_top_folder_stripped() {
    let dir = scratch("tar-gz");
    for name in ["runtime.tar.gz", "runtime.tgz"] {
        let archive = dir.join(name);
        fs::write(
            &archive,
            tar_gz(&[("top/lib/libx.so.1", b"x"), ("top/LICENSE", b"l")]),
        )
        .unwrap();
        let target = dir.join(format!("{name}-out"));
        unpack(&archive, &target).unwrap();
        assert_eq!(fs::read(target.join("lib/libx.so.1")).unwrap(), b"x");
        assert_eq!(fs::read(target.join("LICENSE")).unwrap(), b"l");
    }
    fs::remove_dir_all(&dir).unwrap();
}
