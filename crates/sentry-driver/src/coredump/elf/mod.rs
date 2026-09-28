//! ELF crash-note parsing and backtrace extraction.

pub mod backtrace_extractor;
pub mod elf_extractor;
pub mod elf_header_parser;
pub mod elf_note_parser;

pub use backtrace_extractor::extract_backtrace;
pub use elf_extractor::{extract_elf_crash_headers, extract_elf_crash_headers_from_file};
pub use elf_header_parser::{locate_note_segments, parse_elf_header, ElfHeaderInfo};
pub use elf_note_parser::{parse_elf_notes, ElfNotesInfo};
