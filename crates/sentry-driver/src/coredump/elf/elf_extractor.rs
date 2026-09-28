//! High-level ELF crash metadata extraction combining bounded decompression and ELF parsing.

use crate::coredump::elf_header_parser::{locate_note_segments, parse_elf_header};
use crate::coredump::elf_note_parser::parse_elf_notes;
use crate::coredump::stream_reader::read_bounded_coredump_bytes;
use sentry_core::error::CoredumpError;
use sentry_core::models::coredump::ElfCrashHeader;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Extracts bounded ELF crash headers and notes from an uncompressed or compressed stream.
///
/// Bounded to decompressing at most 64 KiB, preventing large memory allocations.
pub fn extract_elf_crash_headers(reader: &mut impl Read) -> Result<ElfCrashHeader, CoredumpError> {
    let bytes = read_bounded_coredump_bytes(reader)?;
    let header_info = parse_elf_header(&bytes)?;
    let note_segments = locate_note_segments(&bytes, &header_info)?;
    let notes = parse_elf_notes(
        &bytes,
        &note_segments,
        header_info.is_64bit(),
        header_info.is_little_endian(),
    );

    Ok(ElfCrashHeader {
        class: header_info.class,
        endian: header_info.endian,
        endianness: header_info.endian,
        machine: header_info.machine,
        architecture: header_info.architecture,
        signal: notes.signal,
        signal_code: notes.signal_code,
        fault_addr: notes.fault_addr,
        pid: notes.pid,
        comm: notes.comm,
        cmdline: notes.cmdline,
        mapped_files: notes.mapped_files,
    })
}

/// Helper function to open and extract crash metadata directly from a coredump file path.
pub fn extract_elf_crash_headers_from_file(path: &Path) -> Result<ElfCrashHeader, CoredumpError> {
    if !path.exists() {
        return Err(CoredumpError::FileNotFound(path.to_path_buf()));
    }
    let mut file = File::open(path)?;
    extract_elf_crash_headers(&mut file)
}
