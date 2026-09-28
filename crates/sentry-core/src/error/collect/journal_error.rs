//! Errors originating from journal stream parsing and ingestion.

use std::io;
use thiserror::Error;

/// Journal stream parsing and handling errors.
#[derive(Debug, Error)]
pub enum JournalError {
    /// Unexpected end-of-file reached while reading journal entry.
    #[error("Unexpected EOF while reading journal stream")]
    UnexpectedEof,

    /// Binary field size exceeds maximum permitted limit.
    #[error("Journal field '{field}' size ({size} bytes) exceeds maximum limit ({max} bytes)")]
    FieldTooLarge {
        /// Name of the oversized field.
        field: String,
        /// Parsed byte length.
        size: usize,
        /// Maximum allowed byte length.
        max: usize,
    },

    /// Missing expected trailing newline after binary payload.
    #[error("Invalid binary field delimiter for '{field}': expected newline (0x0A), found 0x{found:02X}")]
    InvalidBinaryDelimiter {
        /// Name of the field.
        field: String,
        /// Expected delimiter byte.
        expected: u8,
        /// Actual byte encountered.
        found: u8,
    },

    /// Corrupted or invalid journal export format line.
    #[error("Invalid journal export line format: {0}")]
    InvalidFormat(String),

    /// Underlying standard I/O error.
    #[error("I/O error during journal parsing: {0}")]
    Io(#[from] io::Error),
}
