//! Pure-Rust bounded streaming decompression reader.
//!
//! Supports `.zst`, `.lz4`, and uncompressed ELF core dumps.
//! Bounded to at most 64 KiB to prevent decompressing huge `PT_LOAD` segments.

use sentry_core::error::CoredumpError;
use std::io::Read;

pub use lz4_flex;

/// Maximum bytes allowed to be decompressed or read from a coredump stream (64 KiB).
pub const MAX_DECOMPRESSED_BYTES: u64 = 65536;

/// Magic bytes for Zstandard format: 0x28, 0xB5, 0x2F, 0xFD (little-endian: 0xFD2FB528).
pub const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

/// Magic bytes for LZ4 frame format: 0x04, 0x22, 0x4D, 0x18 (little-endian: 0x184D2204).
pub const LZ4_FRAME_MAGIC: [u8; 4] = [0x04, 0x22, 0x4D, 0x18];

/// Magic bytes for ELF header: 0x7F, 'E', 'L', 'F'.
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// Wrapper providing bounded streaming decompression.
pub struct BoundedStreamReader;

impl BoundedStreamReader {
    /// Reads at most 64 KiB of decompressed bytes from the given reader.
    pub fn read_bounded(reader: &mut impl Read) -> Result<Vec<u8>, CoredumpError> {
        read_bounded_coredump_bytes(reader)
    }

    /// Wraps reader in a bounded decompression reader implementing `std::io::Read`.
    pub fn open_reader<'a>(reader: Box<dyn Read + 'a>) -> Result<Box<dyn Read + 'a>, CoredumpError> {
        open_bounded_decompressed_reader(reader)
    }
}

/// Opens a bounded decompression reader reading at most 64 KiB.
pub fn open_bounded_decompressed_reader<'a>(
    mut reader: Box<dyn Read + 'a>,
) -> Result<Box<dyn Read + 'a>, CoredumpError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).map_err(|e| {
        CoredumpError::ParseError(format!("Input stream too short for magic identification: {e}"))
    })?;

    let chained = std::io::Cursor::new(magic).chain(reader);

    if magic == ZSTD_MAGIC {
        let decoder = ruzstd::StreamingDecoder::new(chained).map_err(|e| {
            CoredumpError::ParseError(format!("Failed to initialize ruzstd decoder: {e:?}"))
        })?;
        Ok(Box::new(decoder.take(MAX_DECOMPRESSED_BYTES)))
    } else if magic == LZ4_FRAME_MAGIC {
        let decoder = lz4_flex::frame::FrameDecoder::new(chained);
        Ok(Box::new(decoder.take(MAX_DECOMPRESSED_BYTES)))
    } else if magic == ELF_MAGIC {
        Ok(Box::new(chained.take(MAX_DECOMPRESSED_BYTES)))
    } else {
        Err(CoredumpError::ParseError(format!(
            "Unrecognized coredump stream format: magic {magic:02X?}"
        )))
    }
}

/// Reads at most 64 KiB (`MAX_DECOMPRESSED_BYTES`) of decompressed or raw ELF bytes.
pub fn read_bounded_coredump_bytes(reader: &mut impl Read) -> Result<Vec<u8>, CoredumpError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).map_err(|e| {
        CoredumpError::ParseError(format!("Input stream too short for magic identification: {e}"))
    })?;

    let chained = std::io::Cursor::new(magic).chain(reader);
    let mut out = Vec::with_capacity(MAX_DECOMPRESSED_BYTES as usize);

    if magic == ZSTD_MAGIC {
        let decoder = ruzstd::StreamingDecoder::new(chained).map_err(|e| {
            CoredumpError::ParseError(format!("Failed to initialize ruzstd decoder: {e:?}"))
        })?;
        let mut bounded = decoder.take(MAX_DECOMPRESSED_BYTES);
        bounded.read_to_end(&mut out)?;
    } else if magic == LZ4_FRAME_MAGIC {
        let decoder = lz4_flex::frame::FrameDecoder::new(chained);
        let mut bounded = decoder.take(MAX_DECOMPRESSED_BYTES);
        bounded.read_to_end(&mut out)?;
    } else if magic == ELF_MAGIC {
        let mut bounded = chained.take(MAX_DECOMPRESSED_BYTES);
        bounded.read_to_end(&mut out)?;
    } else {
        return Err(CoredumpError::ParseError(format!(
            "Unrecognized coredump stream format: magic {magic:02X?}"
        )));
    }

    out.shrink_to_fit();
    Ok(out)
}
