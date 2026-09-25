//! Raw little-endian `f32` files: the decoded mix and the separated stems in the work directory.

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;

use super::{PcmStream, samples_from_le};
use crate::MediaError;

/// Write every chunk of `stream` to `path`, returning the number of samples written.
///
/// The file is written to `<path>.part` and renamed once FFmpeg has finished cleanly, so a
/// complete-looking file is never a cut-short decode.
pub fn write_f32_file(mut stream: PcmStream, path: &Path) -> Result<u64, MediaError> {
    let io = |e: std::io::Error| MediaError::Parse(format!("{}: {e}", path.display()));
    let mut part = path.as_os_str().to_owned();
    part.push(".part");
    let part = std::path::PathBuf::from(part);
    let mut sink = BufWriter::new(File::create(&part).map_err(io)?);
    let mut written = 0u64;
    while let Some(chunk) = stream.next_chunk() {
        let chunk = chunk?;
        for sample in &chunk {
            sink.write_all(&sample.to_le_bytes()).map_err(io)?;
        }
        written += chunk.len() as u64;
    }
    sink.flush().map_err(io)?;
    drop(sink);
    stream.finish()?;
    std::fs::rename(&part, path).map_err(io)?;
    Ok(written)
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
