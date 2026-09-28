use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use super::closure::is_host_library;
use super::*;

/// A minimal 64-bit little-endian shared library: a `.dynstr`, a `.dynamic` with the given
/// NEEDED names, SONAME and RUNPATH, and a `.shstrtab`.
fn tiny_library(soname: &str, needed: &[&str], runpath: &str) -> Vec<u8> {
    let mut dynstr = vec![0u8];
    let mut add = |text: &str| {
        let at = dynstr.len() as u64;
        dynstr.extend_from_slice(text.as_bytes());
        dynstr.push(0);
        at
    };
    let mut entries: Vec<(u64, u64)> = needed.iter().map(|n| (1, add(n))).collect();
    entries.push((14, add(soname)));
    entries.push((29, add(runpath)));
    entries.push((0, 0));
    let shstrtab = b"\0.dynstr\0.dynamic\0.shstrtab\0";
    let dynstr_at = 64u64;
    let dynamic_at = (dynstr_at + dynstr.len() as u64).next_multiple_of(8);
    let shstrtab_at = dynamic_at + 16 * entries.len() as u64;
    let sections_at = (shstrtab_at + shstrtab.len() as u64).next_multiple_of(8);

    let mut out = vec![0x7f, b'E', b'L', b'F', 2, 1, 1];
    out.resize(16, 0);
    let half = |out: &mut Vec<u8>, v: u16| out.extend_from_slice(&v.to_le_bytes());
    let word = |out: &mut Vec<u8>, v: u32| out.extend_from_slice(&v.to_le_bytes());
    let long = |out: &mut Vec<u8>, v: u64| out.extend_from_slice(&v.to_le_bytes());
    half(&mut out, 3); // ET_DYN
    half(&mut out, 62); // EM_X86_64
    word(&mut out, 1);
    long(&mut out, 0); // entry
    long(&mut out, 0); // phoff
    long(&mut out, sections_at);
    word(&mut out, 0); // flags
    for v in [64, 56, 0, 64, 4, 3] {
        half(&mut out, v);
    }
    out.extend_from_slice(&dynstr);
    out.resize(dynamic_at as usize, 0);
    for (tag, val) in &entries {
        long(&mut out, *tag);
        long(&mut out, *val);
    }
    out.extend_from_slice(shstrtab);
    out.resize(sections_at as usize, 0);
    let mut section = |name: u32, kind: u32, offset: u64, size: u64, link: u32, entsize: u64| {
        word(&mut out, name);
        word(&mut out, kind);
        long(&mut out, 0); // flags
        long(&mut out, 0); // addr
        long(&mut out, offset);
        long(&mut out, size);
        word(&mut out, link);
        word(&mut out, 0); // info
        long(&mut out, 1); // addralign
        long(&mut out, entsize);
    };
    section(0, 0, 0, 0, 0, 0);
    section(1, 3, dynstr_at, dynstr.len() as u64, 0, 0);
    section(9, 6, dynamic_at, 16 * entries.len() as u64, 1, 16);
    section(18, 3, shstrtab_at, shstrtab.len() as u64, 0, 0);
    out
}

/// A fresh, empty folder under the system temp folder.
fn scratch(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("appimage_builder-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn reads_needed_soname_and_runpath() {
    let data = tiny_library(
        "libone.so.1",
        &["libc.so.6", "libtwo.so.2"],
        "/a/long/build/dir",
    );
    let dynamic = parse_dynamic(&data).unwrap();
    assert_eq!(dynamic.needed, ["libc.so.6", "libtwo.so.2"]);
    assert_eq!(dynamic.soname.as_deref(), Some("libone.so.1"));
    assert_eq!(dynamic.runpath.unwrap().0, "/a/long/build/dir");
}

#[test]
fn reads_this_test_binary() {
    let exe = std::env::current_exe().unwrap();
    let dynamic = read_dynamic(&exe).unwrap();
    assert!(dynamic.needed.iter().any(|n| n == "libc.so.6"));
}

#[test]
fn origin_expands_in_search_dirs() {
    let data = tiny_library("libone.so.1", &[], "$ORIGIN:$ORIGIN/../lib");
    let dirs = parse_dynamic(&data)
        .unwrap()
        .search_dirs(std::path::Path::new("/x/bin"));
    assert_eq!(
        dirs,
        [PathBuf::from("/x/bin"), PathBuf::from("/x/bin/../lib")]
    );
}

#[test]
fn rewrites_a_runpath_in_place_and_refuses_a_longer_one() {
    let dir = scratch("runpath");
    let path = dir.join("libone.so.1");
    let original = tiny_library("libone.so.1", &["libc.so.6"], "/a/long/build/dir");
    fs::write(&path, &original).unwrap();
    rewrite_runpath(&path, "$ORIGIN").unwrap();
    let rewritten = fs::read(&path).unwrap();
    assert_eq!(rewritten.len(), original.len());
    let dynamic = parse_dynamic(&rewritten).unwrap();
    assert_eq!(dynamic.runpath.unwrap().0, "$ORIGIN");
    assert_eq!(dynamic.soname.as_deref(), Some("libone.so.1"));
    assert!(rewrite_runpath(&path, "$ORIGIN/../../much/longer/than/before").is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn closure_follows_runpath_and_skips_host_and_provided_libraries() {
    let dir = scratch("closure");
    let libs = dir.join("libs");
    fs::create_dir_all(&libs).unwrap();
    let root = dir.join("worker");
    fs::write(
        &root,
        tiny_library(
            "worker",
            &["libc.so.6", "libone.so.1", "libcudart.so.13"],
            "$ORIGIN/libs",
        ),
    )
    .unwrap();
    fs::write(
        libs.join("libone.so.1"),
        tiny_library("libone.so.1", &["libtwo.so.2"], "$ORIGIN"),
    )
    .unwrap();
    fs::write(
        libs.join("libtwo.so.2"),
        tiny_library("libtwo.so.2", &["libm.so.6"], "$ORIGIN"),
    )
    .unwrap();
    let provided = BTreeSet::from(["libcudart.so.13".to_string()]);
    let search = Search {
        dirs: Vec::new(),
        provided: &provided,
    };
    let found = closure(std::slice::from_ref(&root), &search).unwrap();
    let names: Vec<&str> = found.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["worker", "libone.so.1", "libtwo.so.2"]);
    assert_eq!(found[1].dir, libs);

    let lonely = BTreeSet::new();
    let strict = Search {
        dirs: Vec::new(),
        provided: &lonely,
    };
    let error = closure(&[root], &strict).unwrap_err().to_string();
    assert!(error.contains("libcudart.so.13"), "{error}");
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn host_libraries_are_recognised_by_stem() {
    assert!(is_host_library("libc.so.6"));
    assert!(is_host_library("libstdc++.so.6"));
    assert!(is_host_library("libcuda.so.1"));
    assert!(is_host_library("ld-linux-x86-64.so.2"));
    assert!(!is_host_library("libcublas.so.13"));
    assert!(!is_host_library("libcudart.so.13"));
}

#[test]
fn copies_a_library_with_its_symlink_chain() {
    let dir = scratch("copy");
    let from = dir.join("from");
    let to = dir.join("to");
    fs::create_dir_all(&from).unwrap();
    fs::write(from.join("libx.so.1.2.3"), b"real").unwrap();
    std::os::unix::fs::symlink("./libx.so.1.2.3", from.join("libx.so.1")).unwrap();
    std::os::unix::fs::symlink("libx.so.1", from.join("libx.so")).unwrap();
    let real = copy_with_links(&from, "libx.so", &to).unwrap();
    assert_eq!(real, to.join("libx.so.1.2.3"));
    assert_eq!(fs::read(&real).unwrap(), b"real");
    assert_eq!(
        fs::read_link(to.join("libx.so")).unwrap(),
        PathBuf::from("libx.so.1")
    );
    assert_eq!(
        fs::read_link(to.join("libx.so.1")).unwrap(),
        PathBuf::from("libx.so.1.2.3")
    );
    copy_with_links(&from, "libx.so.1", &to).unwrap();
    fs::remove_dir_all(dir).unwrap();
}
