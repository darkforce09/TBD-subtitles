use std::io::{BufReader, Read, Write};
use std::os::unix::fs::symlink;

use backhand::{FilesystemReader, InnerNode};

use super::*;

#[test]
fn writes_an_image_after_a_head_that_reads_back_with_modes_and_links() {
    let dir =
        std::env::temp_dir().join(format!("appimage_builder-squashfs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let tree = dir.join("AppDir");
    fs::create_dir_all(tree.join("usr/bin")).unwrap();
    fs::write(tree.join("usr/bin/tool"), b"#tool body").unwrap();
    fs::set_permissions(tree.join("usr/bin/tool"), fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(tree.join("readme"), b"text").unwrap();
    symlink("usr/bin/tool", tree.join("AppRun")).unwrap();

    let image = dir.join("image");
    let head = b"RUNTIME-BYTES";
    let mut out = File::create(&image).unwrap();
    out.write_all(head).unwrap();
    let size = write_image(&tree, &mut out, head.len() as u64).unwrap();
    drop(out);
    assert!(size > 0);

    let file = BufReader::new(File::open(&image).unwrap());
    let reader = FilesystemReader::from_reader_with_offset(file, head.len() as u64).unwrap();
    let mut seen = Vec::new();
    for node in reader.files() {
        let path = node.fullpath.to_string_lossy().into_owned();
        match &node.inner {
            InnerNode::File(file) if path == "/usr/bin/tool" => {
                assert_eq!(node.header.permissions, 0o755);
                assert_eq!((node.header.uid, node.header.gid), (0, 0));
                let mut body = Vec::new();
                reader.file(file).reader().read_to_end(&mut body).unwrap();
                assert_eq!(body, b"#tool body");
            }
            InnerNode::Symlink(link) if path == "/AppRun" => {
                assert_eq!(link.link, PathBuf::from("usr/bin/tool"));
            }
            _ => {}
        }
        seen.push(path);
    }
    for expected in ["/AppRun", "/readme", "/usr", "/usr/bin", "/usr/bin/tool"] {
        assert!(
            seen.iter().any(|p| p == expected),
            "{expected} missing from {seen:?}"
        );
    }
    fs::remove_dir_all(dir).unwrap();
}
