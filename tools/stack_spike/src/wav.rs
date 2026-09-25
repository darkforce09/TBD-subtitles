//! 16-bit mono WAV excerpts of a 16 kHz `.f32` stem, for listening to a stem by ear.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const RATE: u32 = 16_000;

/// Write `length_s` seconds of `stem` starting at `start_s` as a 16 kHz 16-bit WAV.
pub(crate) fn write_excerpt(
    stem: &Path,
    start_s: f64,
    length_s: f64,
    out: &Path,
) -> anyhow::Result<()> {
    let mut file = std::fs::File::open(stem)?;
    file.seek(SeekFrom::Start((start_s * RATE as f64) as u64 * 4))?;
    let mut bytes = vec![0u8; (length_s * RATE as f64) as usize * 4];
    let mut filled = 0;
    while filled < bytes.len() {
        let n = file.read(&mut bytes[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    bytes.truncate(filled - filled % 4);
    let pcm: Vec<u8> = bytes
        .chunks_exact(4)
        .flat_map(|b| {
            let s = f32::from_le_bytes([b[0], b[1], b[2], b[3]]).clamp(-1.0, 1.0);
            ((s * 32_767.0) as i16).to_le_bytes()
        })
        .collect();
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(&pcm);
    std::fs::File::create(out)?.write_all(&wav)?;
    Ok(())
}
