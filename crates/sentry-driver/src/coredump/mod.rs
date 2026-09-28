//! Pure Rust systemd-coredump crash and backtrace extraction.

pub mod elf;
pub mod scan;

pub use elf::backtrace_extractor;
pub use elf::elf_extractor;
pub use elf::elf_header_parser;
pub use elf::elf_note_parser;
pub use scan::dir_scanner;
pub use scan::journal_matcher;
pub use scan::stream_reader;
pub use scan::xattr_reader;

pub use elf::backtrace_extractor::extract_backtrace;
pub use elf::elf_extractor::{extract_elf_crash_headers, extract_elf_crash_headers_from_file};
pub use elf::elf_header_parser::{locate_note_segments, parse_elf_header, ElfHeaderInfo};
pub use elf::elf_note_parser::{parse_elf_notes, ElfNotesInfo};
pub use scan::dir_scanner::find_latest_coredump;
pub use scan::journal_matcher::match_coredump_record;
pub use scan::stream_reader::{
    lz4_flex, open_bounded_decompressed_reader, read_bounded_coredump_bytes, BoundedStreamReader,
    MAX_DECOMPRESSED_BYTES,
};
pub use scan::xattr_reader::read_coredump_xattrs;
pub use sentry_core::models::coredump::{CoredumpRecord, CoredumpXattrs, ElfCrashHeader};
