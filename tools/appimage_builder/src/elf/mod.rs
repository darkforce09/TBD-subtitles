//! The dynamic section of an ELF binary or library: what it NEEDs, where it looks, what it is.
//!
//! **Role:** read the NEEDED, SONAME and RUNPATH (or older RPATH) entries of a 64-bit ELF file,
//! and where in the file the RUNPATH text lies, for the closure walk and the in-place rewrite.
//!
//! **Position:** used by `gpu_runtime`, `build` and `main` through `closure`, `library_copy` and
//! `runpath`; reads files only, with the `object` crate.
//!
//! **Signals and state:** reads the file it is given; holds nothing.
//!
//! **Invariants:** a file that is not a 64-bit ELF, or whose dynamic section cannot be read, is
//! an error, never an empty answer; the RUNPATH location is a byte range inside the file.

mod closure;
mod library_copy;
mod runpath;

pub(crate) use closure::{Search, closure};
pub(crate) use library_copy::copy_with_links;
pub(crate) use runpath::rewrite_runpath;

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use object::Endianness;
use object::elf::{DT_NEEDED, DT_RPATH, DT_RUNPATH, DT_SONAME, FileHeader64};
use object::read::elf::{Dyn, FileHeader, SectionHeader};

/// The dynamic entries of one ELF file.
#[derive(Debug, Default)]
pub(crate) struct Dynamic {
    /// The libraries the file NEEDs, by soname, in file order.
    pub needed: Vec<String>,
    /// The file's own soname, when it has one.
    pub soname: Option<String>,
    /// The RUNPATH (or RPATH) text and its byte offset in the file.
    pub runpath: Option<(String, u64)>,
}

impl Dynamic {
    /// The RUNPATH entries, with `$ORIGIN` replaced by `origin`.
    pub(crate) fn search_dirs(&self, origin: &Path) -> Vec<std::path::PathBuf> {
        let Some((text, _)) = &self.runpath else {
            return Vec::new();
        };
        let origin = origin.to_string_lossy();
        text.split(':')
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                entry
                    .replace("${ORIGIN}", &origin)
                    .replace("$ORIGIN", &origin)
                    .into()
            })
            .collect()
    }
}

/// Read the dynamic entries of the ELF file at `path`.
pub(crate) fn read_dynamic(path: &Path) -> Result<Dynamic> {
    let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_dynamic(&data)
        .with_context(|| format!("reading the ELF dynamic section of {}", path.display()))
}

/// Read the dynamic entries of an ELF file held in memory.
pub(crate) fn parse_dynamic(data: &[u8]) -> Result<Dynamic> {
    let header = FileHeader64::<Endianness>::parse(data).map_err(|e| anyhow!("{e}"))?;
    let endian = header.endian().map_err(|e| anyhow!("{e}"))?;
    let sections = header.sections(endian, data).map_err(|e| anyhow!("{e}"))?;
    let Some((entries, link)) = sections.dynamic(endian, data).map_err(|e| anyhow!("{e}"))? else {
        return Ok(Dynamic::default());
    };
    let strings = sections
        .strings(endian, data, link)
        .map_err(|e| anyhow!("{e}"))?;
    let strtab_offset = sections
        .section(link)
        .map_err(|e| anyhow!("{e}"))?
        .sh_offset(endian);
    let mut dynamic = Dynamic::default();
    for entry in entries {
        let Some(tag) = entry.tag32(endian) else {
            continue;
        };
        if ![DT_NEEDED, DT_SONAME, DT_RUNPATH, DT_RPATH].contains(&tag) {
            continue;
        }
        let bytes = entry.string(endian, strings).map_err(|e| anyhow!("{e}"))?;
        let text = String::from_utf8_lossy(bytes).into_owned();
        match tag {
            DT_NEEDED => dynamic.needed.push(text),
            DT_SONAME => dynamic.soname = Some(text),
            // RUNPATH wins over RPATH when a file carries both, as the loader does.
            DT_RUNPATH => dynamic.runpath = Some((text, strtab_offset + entry.d_val(endian))),
            _ if dynamic.runpath.is_none() => {
                dynamic.runpath = Some((text, strtab_offset + entry.d_val(endian)));
            }
            _ => {}
        }
    }
    Ok(dynamic)
}

#[cfg(test)]
#[path = "tests/elf.rs"]
mod tests;
