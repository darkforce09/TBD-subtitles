//! Raw little-endian `f32` files: the decoded mix and the separated stems in the work directory.
//!
//! **Role:** write a PCM stream or any run of samples to a raw `.f32` file, and read one back in
//! fixed-size chunks.
//!
//! **Position:** used by the decode and separation stages and the stack spike tool.
//!
//! **Signals and state:** one open file per writer or reader.
//!
//! **Invariants:** a writer fills `<path>.part` and renames it to `path` only on `finish`, so a
//! file at `path` is always whole.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use super::{PcmStream, samples_from_le};
use crate::MediaError;

/// Write every chunk of `stream` to `path`, returning the number of samples written.
///
/// The file is renamed into place only once FFmpeg has finished cleanly, so a complete-looking
/// file is never a cut-short decode.
pub fn write_f32_file(mut stream: PcmStream, path: &Path) -> Result<u64, MediaError> {
    let mut sink = F32FileWriter::create(path)?;
    while let Some(chunk) = stream.next_chunk() {
        sink.write(&chunk?)?;
    }
    stream.finish()?;
    sink.finish()
}

/// Writes a raw `f32` file to `<path>.part` and renames it to `path` on `finish`, so a file at
/// `path` is always whole.
pub struct F32FileWriter {
    sink: BufWriter<File>,
    part: PathBuf,
    path: PathBuf,
    written: u64,
}

impl F32FileWriter {
    pub fn create(path: &Path) -> Result<F32FileWriter, MediaError> {
        let mut part = path.as_os_str().to_owned();
        part.push(".part");
        let part = PathBuf::from(part);
        let file = File::create(&part).map_err(|e| io_error(&part, e))?;
        Ok(F32FileWriter {
            sink: BufWriter::new(file),
            part,
            path: path.to_path_buf(),
            written: 0,
        })
    }

    pub fn write(&mut self, samples: &[f32]) -> Result<(), MediaError> {
        for sample in samples {
            self.sink
                .write_all(&sample.to_le_bytes())
                .map_err(|e| io_error(&self.part, e))?;
        }
        self.written += samples.len() as u64;
        Ok(())
    }

    /// Flush, rename into place, and return the number of samples written.
    pub fn finish(mut self) -> Result<u64, MediaError> {
        self.sink.flush().map_err(|e| io_error(&self.part, e))?;
        std::fs::rename(&self.part, &self.path).map_err(|e| io_error(&self.path, e))?;
        Ok(self.written)
    }
}

fn io_error(path: &Path, e: std::io::Error) -> MediaError {
    MediaError::Parse(format!("{}: {e}", path.display()))
}

/// Reads a raw `f32` file back in fixed-size chunks.
pub struct F32FileReader {
    source: BufReader<File>,
    chunk_samples: usize,
}

impl F32FileReader {
    pub fn open(path: &Path, chunk_samples: usize) -> Result<F32FileReader, MediaError> {
        let file =
            File::open(path).map_err(|e| MediaError::Parse(format!("{}: {e}", path.display())))?;
        Ok(F32FileReader {
            source: BufReader::new(file),
            chunk_samples: chunk_samples.max(1),
        })
    }

    /// The number of samples in a raw `f32` file.
    pub fn sample_count(path: &Path) -> Result<u64, MediaError> {
        std::fs::metadata(path)
            .map(|m| m.len() / 4)
            .map_err(|e| MediaError::Parse(format!("{}: {e}", path.display())))
    }

    /// The next chunk; the last one may be shorter.
    pub fn next_chunk(&mut self) -> Result<Option<Vec<f32>>, MediaError> {
        let mut bytes = vec![0u8; self.chunk_samples * 4];
        let mut filled = 0;
        while filled < bytes.len() {
            let n = self
                .source
                .read(&mut bytes[filled..])
                .map_err(|e| MediaError::Parse(e.to_string()))?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        if filled == 0 {
            return Ok(None);
        }
        bytes.truncate(filled - filled % 4);
        Ok(Some(samples_from_le(&bytes)))
    }
}
