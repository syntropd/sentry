//! 1:1 Unit QA tests for ELF header and program header parsing.

use sentry_driver::coredump::elf_header_parser::{
    locate_note_segments, parse_elf_header, PT_NOTE,
};

#[test]
fn test_parse_elf64_little_endian() {
    let mut buf = vec![0u8; 64];
    buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    buf[4] = 2; // 64-bit
    buf[5] = 1; // Little-endian
    buf[18..20].copy_from_slice(&0x3E_u16.to_le_bytes()); // x86_64
    buf[32..40].copy_from_slice(&64_u64.to_le_bytes()); // phoff
    buf[54..56].copy_from_slice(&56_u16.to_le_bytes()); // phentsize
    buf[56..58].copy_from_slice(&2_u16.to_le_bytes()); // phnum

    let info = parse_elf_header(&buf).unwrap();
    assert_eq!(info.class, 2);
    assert_eq!(info.endian, 1);
    assert!(info.is_64bit());
    assert!(info.is_little_endian());
    assert_eq!(info.machine, 0x3E);
    assert_eq!(info.architecture, "x86_64");
    assert_eq!(info.phoff, 64);
    assert_eq!(info.phentsize, 56);
    assert_eq!(info.phnum, 2);
}

#[test]
fn test_parse_elf64_big_endian() {
    let mut buf = vec![0u8; 64];
    buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    buf[4] = 2; // 64-bit
    buf[5] = 2; // Big-endian
    buf[18..20].copy_from_slice(&0x15_u16.to_be_bytes()); // ppc64
    buf[32..40].copy_from_slice(&64_u64.to_be_bytes()); // phoff
    buf[54..56].copy_from_slice(&56_u16.to_be_bytes()); // phentsize
    buf[56..58].copy_from_slice(&1_u16.to_be_bytes()); // phnum

    let info = parse_elf_header(&buf).unwrap();
    assert_eq!(info.class, 2);
    assert_eq!(info.endian, 2);
    assert!(!info.is_little_endian());
    assert_eq!(info.machine, 0x15);
    assert_eq!(info.architecture, "ppc64");
}

#[test]
fn test_parse_elf32_little_endian() {
    let mut buf = vec![0u8; 52];
    buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    buf[4] = 1; // 32-bit
    buf[5] = 1; // Little-endian
    buf[18..20].copy_from_slice(&0x03_u16.to_le_bytes()); // x86
    buf[28..32].copy_from_slice(&52_u32.to_le_bytes()); // phoff
    buf[42..44].copy_from_slice(&32_u16.to_le_bytes()); // phentsize
    buf[44..46].copy_from_slice(&1_u16.to_le_bytes()); // phnum

    let info = parse_elf_header(&buf).unwrap();
    assert_eq!(info.class, 1);
    assert!(!info.is_64bit());
    assert!(info.is_little_endian());
    assert_eq!(info.machine, 0x03);
    assert_eq!(info.architecture, "x86");
    assert_eq!(info.phoff, 52);
    assert_eq!(info.phentsize, 32);
    assert_eq!(info.phnum, 1);
}

#[test]
fn test_parse_elf_invalid_magic() {
    let mut buf = vec![0u8; 64];
    buf[0..4].copy_from_slice(&[0x00, 0x01, 0x02, 0x03]);
    let err = parse_elf_header(&buf).unwrap_err();
    assert!(err.to_string().contains("Invalid ELF magic bytes"));
}

#[test]
fn test_parse_elf_truncated_header() {
    let buf = vec![0x7F, b'E', b'L', b'F', 2, 1];
    let err = parse_elf_header(&buf).unwrap_err();
    assert!(err.to_string().contains("Buffer too small"));
}

#[test]
fn test_locate_note_segments_64bit() {
    let mut buf = vec![0u8; 64 + 56 * 2];
    // ELF Header
    buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    buf[4] = 2; // 64-bit
    buf[5] = 1; // LE
    buf[18..20].copy_from_slice(&0x3E_u16.to_le_bytes());
    buf[32..40].copy_from_slice(&64_u64.to_le_bytes());
    buf[54..56].copy_from_slice(&56_u16.to_le_bytes());
    buf[56..58].copy_from_slice(&2_u16.to_le_bytes());

    // PH 0: PT_NOTE
    let ph0 = 64;
    buf[ph0..ph0 + 4].copy_from_slice(&PT_NOTE.to_le_bytes());
    buf[ph0 + 8..ph0 + 16].copy_from_slice(&1000_u64.to_le_bytes()); // offset
    buf[ph0 + 32..ph0 + 40].copy_from_slice(&2500_u64.to_le_bytes()); // size

    // PH 1: PT_LOAD (type 1)
    let ph1 = 64 + 56;
    buf[ph1..ph1 + 4].copy_from_slice(&1_u32.to_le_bytes());

    let header_info = parse_elf_header(&buf).unwrap();
    let notes = locate_note_segments(&buf, &header_info).unwrap();

    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0], (1000, 2500));
}
