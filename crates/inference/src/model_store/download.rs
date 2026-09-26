//! Downloading one pinned file over https, resuming a partial download and checking its hash.
//!
//! **Role:** fetch a URL into `<dest>.part`, continue an interrupted `.part` with an HTTP range
//! request, check the finished file's size and SHA-256, and only then rename it into place.
//!
//! **Position:** called by `mod.rs` for model files and by `archive.rs` for runtime archives.
//!
//! **Signals and state:** reads and writes the destination folder; network access through ureq.
//!
//! **Invariants:** a file at its final path always matches its pinned size and hash; a mismatch
//! deletes the `.part` so the next attempt starts clean.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::StoreError;

/// Bytes read per network read and hash update.
const BLOCK: usize = 1 << 20;

/// What a download reports as it goes: bytes held so far and the total. Breaking stops the
/// download with [`StoreError::Cancelled`]; the part file stays, so the next attempt resumes it.
pub type Progress<'a> = &'a mut dyn FnMut(u64, u64) -> ControlFlow<()>;

/// Download `url` to `dest` unless a file matching `size` and `sha256` is already there.
pub fn fetch_verified(
    url: &str,
    dest: &Path,
    size: u64,
    sha256: &str,
    progress: Progress<'_>,
) -> Result<(), StoreError> {
    if dest.is_file() && file_size(dest)? == size {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| StoreError::io(parent, e))?;
    }
    let part = part_path(dest);
    let mut hasher = Sha256::new();
    let mut held = if part.is_file() {
        hash_existing(&part, &mut hasher)?
    } else {
        0
    };
    if held > size {
        fs::remove_file(&part).map_err(|e| StoreError::io(&part, e))?;
        held = 0;
        hasher = Sha256::new();
    }
    if held < size {
        held = stream_into(url, &part, held, &mut hasher, size, progress)?;
    }
    if held != size {
        let _ = fs::remove_file(&part);
        return Err(StoreError::SizeMismatch {
            path: dest.to_path_buf(),
            expected: size,
            found: held,
        });
    }
    let found = hex(&hasher.finalize());
    if found != sha256 {
        let _ = fs::remove_file(&part);
        return Err(StoreError::ChecksumMismatch {
            path: dest.to_path_buf(),
            expected: sha256.to_string(),
            found,
        });
    }
    fs::rename(&part, dest).map_err(|e| StoreError::io(dest, e))
}

/// Hash a whole file, for checks of files already in place.
pub fn sha256_of(path: &Path) -> Result<String, StoreError> {
    let mut hasher = Sha256::new();
    hash_existing(path, &mut hasher)?;
    Ok(hex(&hasher.finalize()))
}

fn stream_into(
    url: &str,
    part: &Path,
    from: u64,
    hasher: &mut Sha256,
    size: u64,
    progress: Progress<'_>,
) -> Result<u64, StoreError> {
    let http = |message: String| StoreError::Http {
        url: url.to_string(),
        message,
    };
    let mut request = ureq::get(url);
    if from > 0 {
        request = request.header("Range", format!("bytes={from}-"));
    }
    let response = request.call().map_err(|e| http(e.to_string()))?;
    let resumed = response.status().as_u16() == 206;
    let mut held = from;
    let mut sink = if resumed {
        OpenOptions::new()
            .append(true)
            .open(part)
            .map_err(|e| StoreError::io(part, e))?
    } else {
        // The server ignored the range: start over.
        *hasher = Sha256::new();
        held = 0;
        File::create(part).map_err(|e| StoreError::io(part, e))?
    };
    let mut reader = response.into_body().into_reader();
    let mut block = vec![0u8; BLOCK];
    loop {
        let n = reader.read(&mut block).map_err(|e| http(e.to_string()))?;
        if n == 0 {
            break;
        }
        sink.write_all(&block[..n])
            .map_err(|e| StoreError::io(part, e))?;
        hasher.update(&block[..n]);
        held += n as u64;
        if held > size {
            break;
        }
        if progress(held, size).is_break() {
            sink.flush().map_err(|e| StoreError::io(part, e))?;
            return Err(StoreError::Cancelled);
        }
    }
    sink.flush().map_err(|e| StoreError::io(part, e))?;
    Ok(held)
}

fn hash_existing(path: &Path, hasher: &mut Sha256) -> Result<u64, StoreError> {
    let mut file = File::open(path).map_err(|e| StoreError::io(path, e))?;
    let mut block = vec![0u8; BLOCK];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut block).map_err(|e| StoreError::io(path, e))?;
        if n == 0 {
            return Ok(total);
        }
        hasher.update(&block[..n]);
        total += n as u64;
    }
}

fn file_size(path: &Path) -> Result<u64, StoreError> {
    fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| StoreError::io(path, e))
}

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

/// Lowercase hex of a digest.
pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0xf) as usize] as char);
    }
    out
}
