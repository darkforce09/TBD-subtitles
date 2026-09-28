//! The libraries a set of ELF files needs, found the way the dynamic loader finds them.
//!
//! **Role:** walk NEEDED from each root, breadth first, resolving each soname in the needing
//! file's own RUNPATH (with `$ORIGIN`) and then in the given folders, and return every library
//! found, once.
//!
//! **Position:** part of `elf`; `gpu_runtime` walks the CUDA, cuDNN and ONNX Runtime libraries
//! with it, and `main` walks the ggml worker.
//!
//! **Signals and state:** reads the files it visits; holds nothing.
//!
//! **Invariants:** a soname that is neither found nor skipped is an error naming the file that
//! needs it; the host's own libraries (glibc, the C++ runtime, the NVIDIA driver) are never
//! bundled.

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::read_dynamic;

/// The libraries the host always supplies, by the name before `.so`: glibc, the GCC runtimes and
/// the NVIDIA driver's own libraries, which must match the installed kernel module.
const HOST_LIBRARIES: &[&str] = &[
    "libc",
    "libm",
    "libdl",
    "libpthread",
    "librt",
    "libutil",
    "libgcc_s",
    "libstdc++",
    "libgomp",
    "libz",
    "libcuda",
    "libnvidia-ml",
    "ld-linux-x86-64",
];

/// Whether `soname` is a library the host supplies, never bundled.
pub(crate) fn is_host_library(soname: &str) -> bool {
    let stem = soname.split(".so").next().unwrap_or(soname);
    HOST_LIBRARIES.contains(&stem)
}

/// How to resolve the sonames of one walk.
pub(crate) struct Search<'a> {
    /// Folders searched after each file's own RUNPATH.
    pub dirs: Vec<PathBuf>,
    /// Sonames provided some other way and left out of the walk, besides the host's.
    pub provided: &'a BTreeSet<String>,
}

/// One library the walk found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    /// The name it was needed by (or the root's file name).
    pub name: String,
    /// The folder it was found in.
    pub dir: PathBuf,
}

/// Every library `roots` need, roots included, each once, in the order found.
pub(crate) fn closure(roots: &[PathBuf], search: &Search<'_>) -> Result<Vec<Found>> {
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    for root in roots {
        let (Some(dir), Some(name)) = (root.parent(), root.file_name()) else {
            bail!("{} names no file", root.display());
        };
        let name = name.to_string_lossy().into_owned();
        if seen.insert(name.clone()) {
            found.push(Found {
                name,
                dir: dir.to_path_buf(),
            });
            queue.push_back(root.clone());
        }
    }
    let mut missing = Vec::new();
    while let Some(file) = queue.pop_front() {
        let dynamic = read_dynamic(&file)?;
        let origin = file.parent().unwrap_or(Path::new("/"));
        let mut dirs = dynamic.search_dirs(origin);
        dirs.extend(search.dirs.iter().cloned());
        for needed in dynamic.needed {
            if is_host_library(&needed) || search.provided.contains(&needed) {
                continue;
            }
            if !seen.insert(needed.clone()) {
                continue;
            }
            match dirs.iter().find(|dir| dir.join(&needed).exists()) {
                Some(dir) => {
                    queue.push_back(dir.join(&needed));
                    found.push(Found {
                        name: needed,
                        dir: dir.clone(),
                    });
                }
                None => missing.push(format!("{needed} (needed by {})", file.display())),
            }
        }
    }
    if !missing.is_empty() {
        bail!("libraries not found: {}", missing.join(", "));
    }
    Ok(found)
}
