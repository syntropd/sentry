//! Adversarial stress tests for ELF program header and note segment extraction.
//!
//! Tests corrupt program headers, malformed note records (u32 overflow, invalid lengths),
//! and empty/missing/non-UTF8 strings in NT_PRPSINFO and NT_FILE.

use sentry_driver::coredump::elf_extractor::extract_elf_crash_headers;
use sentry_driver::coredump::elf_header_parser::{locate_note_segments, parse_elf_header, PT_NOTE};
use sentry_driver::coredump::elf_note_parser::{
    parse_elf_notes, NT_FILE, NT_PRPSINFO, NT_PRSTATUS, NT_SIGINFO,
};
use std::io::Cursor;

fn build_base_elf64_header() -> Vec<u8> {
    let mut elf = vec![0u8; 64];
    elf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    elf[4] = 2; // 64-bit
    elf[5] = 1; // LE
    elf[18..20].copy_from_slice(&0x3E_u16.to_le_bytes()); // x86_64
    elf[32..40].copy_from_slice(&64_u64.to_le_bytes()); // phoff = 64
    elf[54..56].copy_from_slice(&56_u16.to_le_bytes()); // phentsize = 56
    elf[56..58].copy_from_slice(&1_u16.to_le_bytes()); // phnum = 1
    elf
}

#[test]
fn test_adversarial_phnum_beyond_eof() {
    let mut elf = build_base_elf64_header();
    // Claim 1000 program headers, but buffer is only 64 bytes (0 PH bytes)
    elf[56..58].copy_from_slice(&1000_u16.to_le_bytes());

    let header = parse_elf_header(&elf).unwrap();
    assert_eq!(header.phnum, 1000);

    let notes = locate_note_segments(&elf, &header).unwrap();
    assert!(notes.is_empty(), "Should cleanly break without panic when phoff is at/beyond EOF");
}

#[test]
fn test_adversarial_phoff_beyond_buffer() {
    let mut elf = build_base_elf64_header();
    elf[32..40].copy_from_slice(&1_000_000_u64.to_le_bytes()); // phoff = 1,000,000

    let header = parse_elf_header(&elf).unwrap();
    let notes = locate_note_segments(&elf, &header).unwrap();
    assert!(notes.is_empty(), "Should cleanly break when phoff is far beyond buffer");
}

#[test]
fn test_adversarial_phoff_near_u64_max() {
    let mut elf = build_base_elf64_header();
    elf[32..40].copy_from_slice(&(u64::MAX - 10).to_le_bytes());

    let header = parse_elf_header(&elf).unwrap();
    let panic_res = std::panic::catch_unwind(|| {
        let _ = locate_note_segments(&elf, &header);
    });
    assert!(
        panic_res.is_ok(),
        "CRITICAL BUG: locate_note_segments panics with overflow when phoff is near u64::MAX"
    );
}

#[test]
fn test_adversarial_corrupt_pt_note_offsets() {
    let mut elf = build_base_elf64_header();
    let mut phdr = vec![0u8; 56];
    phdr[0..4].copy_from_slice(&PT_NOTE.to_le_bytes()); // PT_NOTE
    phdr[8..16].copy_from_slice(&0xDEAD_BEEF_u64.to_le_bytes()); // corrupt p_offset
    phdr[32..40].copy_from_slice(&0xFFFF_FFFF_u64.to_le_bytes()); // corrupt p_filesz
    elf.extend_from_slice(&phdr);

    let mut cursor = Cursor::new(elf);
    let crash_header = extract_elf_crash_headers(&mut cursor).unwrap();
    // Notes beyond bounds should be safely skipped, leaving header fields None
    assert_eq!(crash_header.architecture, "x86_64");
    assert_eq!(crash_header.signal, None);
    assert_eq!(crash_header.pid, None);
}

#[test]
fn test_adversarial_note_header_truncated_under_12_bytes() {
    let corrupt_bytes = vec![0x01, 0x02, 0x03, 0x04];
    let segs = [(0_u64, corrupt_bytes.len() as u64)];
    let info = parse_elf_notes(&corrupt_bytes, &segs, true, true);
    assert_eq!(info.signal, None);
    assert_eq!(info.pid, None);
}

#[test]
fn test_adversarial_note_namesz_descsz_u32_max() {
    let mut note = Vec::new();
    note.extend_from_slice(&u32::MAX.to_le_bytes()); // namesz = 0xFFFFFFFF
    note.extend_from_slice(&u32::MAX.to_le_bytes()); // descsz = 0xFFFFFFFF
    note.extend_from_slice(&1_u32.to_le_bytes()); // ntype = NT_PRSTATUS
    note.extend_from_slice(&[0u8; 32]);

    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, true, true);
    assert_eq!(info.signal, None);
    assert_eq!(info.pid, None);
}

#[test]
fn test_adversarial_note_pointing_past_slice() {
    let mut note = Vec::new();
    note.extend_from_slice(&100_u32.to_le_bytes()); // namesz = 100
    note.extend_from_slice(&200_u32.to_le_bytes()); // descsz = 200
    note.extend_from_slice(&NT_PRSTATUS.to_le_bytes());
    // Only 8 bytes of actual data follows (less than namesz + descsz)
    note.extend_from_slice(&[0xAA; 8]);

    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, true, true);
    assert_eq!(info.signal, None);
}

#[test]
fn test_adversarial_note_zero_length_loop_termination() {
    let mut note = Vec::new();
    // 3 empty notes in a row
    for _ in 0..3 {
        note.extend_from_slice(&0_u32.to_le_bytes()); // namesz = 0
        note.extend_from_slice(&0_u32.to_le_bytes()); // descsz = 0
        note.extend_from_slice(&99_u32.to_le_bytes()); // unknown type
    }
    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, true, true);
    assert_eq!(info.signal, None);
}

#[test]
fn test_adversarial_prpsinfo_all_zeroes() {
    let mut note = Vec::new();
    let namesz = 4_u32;
    let descsz = 136_u32;
    note.extend_from_slice(&namesz.to_le_bytes());
    note.extend_from_slice(&descsz.to_le_bytes());
    note.extend_from_slice(&NT_PRPSINFO.to_le_bytes());
    note.extend_from_slice(b"CORE");
    note.extend_from_slice(&vec![0u8; 136]); // all zeroes

    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, true, true);
    // PID 0 is ignored, empty strings ignored
    assert_eq!(info.pid, None);
    assert_eq!(info.comm, None);
    assert_eq!(info.cmdline, None);
}

#[test]
fn test_adversarial_prpsinfo_non_utf8_strings() {
    let mut note = Vec::new();
    let namesz = 4_u32;
    let descsz = 136_u32;
    note.extend_from_slice(&namesz.to_le_bytes());
    note.extend_from_slice(&descsz.to_le_bytes());
    note.extend_from_slice(&NT_PRPSINFO.to_le_bytes());
    note.extend_from_slice(b"CORE");

    let mut desc = vec![0u8; 136];
    desc[24..28].copy_from_slice(&999_u32.to_le_bytes()); // pid = 999
    // Invalid UTF-8 bytes in comm
    desc[40..44].copy_from_slice(&[0xFF, 0xFE, 0xFD, 0x00]);
    // Invalid UTF-8 bytes in cmdline
    desc[56..60].copy_from_slice(&[0xC0, 0xAF, 0x80, 0x00]);

    note.extend_from_slice(&desc);

    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, true, true);
    assert_eq!(info.pid, Some(999));
    // Non-UTF8 strings should safely decode to empty and be ignored
    assert_eq!(info.comm, None);
    assert_eq!(info.cmdline, None);
}

#[test]
fn test_adversarial_nt_file_corrupt_count_and_empty_strings() {
    let mut note = Vec::new();
    let namesz = 4_u32;
    let descsz = 64_u32;
    note.extend_from_slice(&namesz.to_le_bytes());
    note.extend_from_slice(&descsz.to_le_bytes());
    note.extend_from_slice(&NT_FILE.to_le_bytes());
    note.extend_from_slice(b"CORE");

    let mut desc = Vec::new();
    desc.extend_from_slice(&1_000_000_u64.to_le_bytes()); // huge count beyond desc size
    desc.extend_from_slice(&4096_u64.to_le_bytes());
    desc.resize(64, 0); // null bytes

    note.extend_from_slice(&desc);

    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, true, true);
    assert!(info.mapped_files.is_empty(), "Corrupt count must not crash and produce empty mapped files");
}

#[test]
fn test_adversarial_nt_file_non_utf8_paths() {
    let mut note = Vec::new();
    let namesz = 4_u32;
    let mut desc = Vec::new();
    desc.extend_from_slice(&1_u64.to_le_bytes()); // count = 1
    desc.extend_from_slice(&4096_u64.to_le_bytes()); // page_size
    desc.extend_from_slice(&0x400000_u64.to_le_bytes());
    desc.extend_from_slice(&0x401000_u64.to_le_bytes());
    desc.extend_from_slice(&0_u64.to_le_bytes());
    // String table: non-utf8 invalid bytes + null + valid path + null
    desc.extend_from_slice(&[0xFF, 0xFE, 0x00]);
    desc.extend_from_slice(b"/usr/bin/valid\0");

    note.extend_from_slice(&namesz.to_le_bytes());
    note.extend_from_slice(&(desc.len() as u32).to_le_bytes());
    note.extend_from_slice(&NT_FILE.to_le_bytes());
    note.extend_from_slice(b"CORE");
    note.extend_from_slice(&desc);

    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, true, true);
    assert_eq!(info.mapped_files, vec!["/usr/bin/valid"]);
}

#[test]
fn test_adversarial_siginfo_negative_values() {
    let mut note = Vec::new();
    let namesz = 4_u32;
    let descsz = 128_u32;
    note.extend_from_slice(&namesz.to_le_bytes());
    note.extend_from_slice(&descsz.to_le_bytes());
    note.extend_from_slice(&NT_SIGINFO.to_le_bytes());
    note.extend_from_slice(b"CORE");

    let mut desc = vec![0u8; 128];
    desc[0..4].copy_from_slice(&(-1_i32).to_le_bytes()); // negative signo
    desc[8..12].copy_from_slice(&(-5_i32).to_le_bytes()); // negative code
    note.extend_from_slice(&desc);

    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, true, true);
    // Negative signal is not set
    assert_eq!(info.signal, None);
    assert_eq!(info.signal_code, Some(-5));
}
